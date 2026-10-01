#!/usr/bin/env python3
"""Check for orphaned Rust modules.

This script walks through each crate's src/ directory, resolves the module tree
from lib.rs (and main.rs if present), and reports any .rs files that are not
reachable from the module declarations. It also checks cargo-fuzz bin roots.

It also flags stray .rs files outside any crate's src/ directory, with an allowlist
for legitimate cases such as build.rs.
"""

import re
import sys
from pathlib import Path

# Allowlist for .rs files that are legitimately outside crate src/ directories
ALLOWLISTED_ROOT_FILES: frozenset[str] = frozenset({"build.rs"})

# Directories to skip (not crates)
SKIP_DIRS: frozenset[str] = frozenset({"target", ".git"})


def extract_mod_declarations(file_path: Path) -> set[str]:
    """Extract module declarations from a Rust source file."""
    mods: set[str] = set()
    try:
        content = file_path.read_text(encoding="utf-8")
        for line in content.splitlines():
            line = line.strip()
            # Skip comments
            if line.startswith("//") or line.startswith("/*"):
                continue
            # Match mod declarations (with optional pub / pub(crate))
            match = re.match(r"(?:pub\s*(?:\(\w+\)\s*)?)?mod\s+(\w+)\s*(?:{|;)", line)
            if match:
                mods.add(match.group(1))
    except OSError as exc:
        print(f"Warning: Could not read {file_path}: {exc}", file=sys.stderr)
    return mods


def resolve_module_tree(
    crate_root: Path,
    visited: set[Path] | None = None,
) -> set[Path]:
    """Recursively resolve all reachable module files from the crate root."""
    if visited is None:
        visited = set()

    if crate_root in visited:
        return set()
    visited.add(crate_root)

    reachable: set[Path] = {crate_root}
    for mod_name in extract_mod_declarations(crate_root):
        mod_rs = crate_root.parent / f"{mod_name}.rs"
        mod_dir = crate_root.parent / mod_name / "mod.rs"

        if mod_rs.exists():
            reachable.add(mod_rs)
            reachable.update(resolve_module_tree(mod_rs, visited))
        elif mod_dir.exists():
            reachable.add(mod_dir)
            reachable.update(resolve_module_tree(mod_dir, visited))

    return reachable


def find_crates(repo_root: Path) -> dict[Path, Path]:
    """Find all crates by looking for Cargo.toml files."""
    crates: dict[Path, Path] = {}
    for cargo_toml in repo_root.rglob("Cargo.toml"):
        if any(skip_dir in cargo_toml.parts for skip_dir in SKIP_DIRS):
            continue
        src_dir = cargo_toml.parent / "src"
        if not src_dir.exists():
            continue
        for entry in ("lib.rs", "main.rs"):
            candidate = src_dir / entry
            if candidate.exists():
                crates[cargo_toml] = candidate
                break
    return crates


def check_crate(crate_root: Path) -> tuple[set[Path], set[Path]]:
    """Check a single crate for orphaned modules."""
    all_rs_files = set((crate_root.parent).rglob("*.rs"))
    reachable = resolve_module_tree(crate_root)
    orphans = {f for f in (all_rs_files - reachable) if not f.name.endswith("_test.rs")}
    return reachable, orphans


def check_fuzz_crate(fuzz_manifest: Path) -> tuple[set[Path], set[Path]]:
    """Check that every fuzz Rust source is reachable from a declared bin."""
    fuzz_dir = fuzz_manifest.parent
    manifest = fuzz_manifest.read_text(encoding="utf-8")
    bin_sections = re.split(r"(?m)^\[\[bin\]\]\s*$", manifest)[1:]
    target_paths = []

    for section in bin_sections:
        path_match = re.search(r'(?m)^\s*path\s*=\s*"([^"]+)"\s*$', section)
        if path_match:
            target_paths.append(fuzz_dir / path_match.group(1))

    reachable: set[Path] = set()
    for target_path in target_paths:
        if target_path.exists():
            reachable.update(resolve_module_tree(target_path))

    all_rs_files = {
        source
        for source in fuzz_dir.rglob("*.rs")
        if "target" not in source.relative_to(fuzz_dir).parts
    }
    return reachable, all_rs_files - reachable


def check_stray_files(repo_root: Path) -> set[Path]:
    """Check for stray .rs files outside crate src/ directories."""
    stray: set[Path] = set()

    for rs_file in repo_root.glob("*.rs"):
        if rs_file.name not in ALLOWLISTED_ROOT_FILES:
            stray.add(rs_file)

    for item in repo_root.iterdir():
        if item.is_dir() and not (item / "Cargo.toml").exists() and item.name not in SKIP_DIRS:
            for rs_file in item.glob("*.rs"):
                if rs_file.name not in ALLOWLISTED_ROOT_FILES:
                    stray.add(rs_file)

    return stray


def main() -> None:
    repo_root = Path(__file__).parent.parent

    print(f"Checking for orphaned modules in {repo_root}...")

    crates = find_crates(repo_root)
    print(f"Found {len(crates)} crates")

    total_orphans = 0
    has_errors = False

    for cargo_toml, crate_root in sorted(crates.items()):
        print(f"\nChecking crate: {cargo_toml.parent}")
        reachable, orphans = check_crate(crate_root)

        if orphans:
            has_errors = True
            print(f"  ERROR: {len(orphans)} orphaned module(s) found:")
            for orphan in sorted(orphans):
                print(f"    - {orphan.relative_to(repo_root)}")
            total_orphans += len(orphans)
        else:
            print(f"  OK: All {len(reachable)} modules are reachable")

    fuzz_manifest = repo_root / "fuzz" / "Cargo.toml"
    if fuzz_manifest.exists():
        print(f"\nChecking fuzz targets: {fuzz_manifest.parent}")
        reachable, orphans = check_fuzz_crate(fuzz_manifest)

        if orphans:
            has_errors = True
            print(f"  ERROR: {len(orphans)} unreachable Rust source(s) found:")
            for orphan in sorted(orphans):
                print(f"    - {orphan.relative_to(repo_root)}")
            total_orphans += len(orphans)
        else:
            print(f"  OK: All {len(reachable)} fuzz sources are reachable")

    stray_files = check_stray_files(repo_root)
    if stray_files:
        has_errors = True
        print(f"\nERROR: {len(stray_files)} stray .rs file(s) found outside crate src/:")
        for stray in sorted(stray_files):
            print(f"  - {stray.relative_to(repo_root)}")
    else:
        print("\nOK: No stray .rs files found outside crate src/")

    if has_errors:
        print(f"\nTotal issues: {total_orphans + len(stray_files)}")
        print("Please either wire in orphaned modules or remove unused files.")
        sys.exit(1)
    else:
        print("\nAll checks passed!")


if __name__ == "__main__":
    main()
