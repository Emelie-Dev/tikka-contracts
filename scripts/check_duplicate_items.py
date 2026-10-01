#!/usr/bin/env python3
"""
Check for duplicate top-level item definitions in Rust source files.

Scans every .rs file reachable via git ls-files (falling back to a recursive
walk) and reports, per file:
  - duplicate fn / struct / enum names at the top level
  - duplicate enum variant discriminants (catches the = 67 collision class)

Exit codes:
  0 — no duplicates found
  1 — at least one duplicate found
"""

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Directories whose contents are never scanned.
SKIP_DIRS = {"target", "fuzz", ".git"}

# Item kinds matched by the top-level duplicate check.
# The regex below intentionally stays simple: it matches declarations that
# begin at column 0 (possibly with a visibility prefix).
ITEM_PATTERN = re.compile(
    r"^(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(fn|struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)"
)

# Enum variant with an explicit integer discriminant, e.g.  `Foo = 42,`
VARIANT_PATTERN = re.compile(r"^\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*,?")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def list_rs_files(repo_root: Path) -> list[Path]:
    """Return all tracked .rs files, or fall back to a recursive glob."""
    try:
        result = subprocess.run(
            ["git", "ls-files", "--", "contracts/**/*.rs"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        )
        paths = [repo_root / p for p in result.stdout.splitlines() if p.strip()]
        if paths:
            return paths
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass

    # Fallback: walk the file tree ourselves.
    paths = []
    for rs in repo_root.rglob("*.rs"):
        if any(skip in rs.parts for skip in SKIP_DIRS):
            continue
        paths.append(rs)
    return paths


def strip_comments(lines: list[str]) -> list[str]:
    """Strip single-line comments. Block comments are left in place
    (keeping things simple; they are rare for top-level items)."""
    out = []
    for line in lines:
        # Remove everything from // to end of line, but not inside strings.
        # Good enough for structural analysis.
        stripped = re.sub(r"//.*$", "", line)
        out.append(stripped)
    return out


def check_file(path: Path) -> tuple[list[str], list[str]]:
    """Return (item_errors, discriminant_errors) for a single file."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"  warning: cannot read {path}: {exc}", file=sys.stderr)
        return [], []

    lines = strip_comments(text.splitlines())

    # ------------------------------------------------------------------
    # 1. Top-level item duplicates (fn / struct / enum)
    # ------------------------------------------------------------------
    item_counts: dict[str, list[int]] = defaultdict(list)
    for lineno, line in enumerate(lines, 1):
        m = ITEM_PATTERN.match(line)
        if m:
            name = m.group(2)
            item_counts[name].append(lineno)

    item_errors = []
    for name, linenos in sorted(item_counts.items()):
        if len(linenos) > 1:
            locs = ", ".join(f"line {n}" for n in linenos)
            item_errors.append(f"  DUPLICATE item '{name}' defined {len(linenos)} times ({locs})")

    # ------------------------------------------------------------------
    # 2. Enum discriminant collisions
    #    Walk enum bodies looking for explicit `= <int>` discriminants.
    # ------------------------------------------------------------------
    discriminant_errors = []
    in_enum = False
    enum_name = ""
    brace_depth = 0
    # discriminant → [(variant_name, lineno), ...]
    discriminants: dict[int, list[tuple[str, int]]] = defaultdict(list)

    for lineno, line in enumerate(lines, 1):
        # Detect entering an enum body
        if not in_enum:
            em = re.match(
                r"^(?:pub(?:\s*\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)",
                line,
            )
            if em:
                enum_name = em.group(1)
                in_enum = True
                brace_depth = 0
                discriminants = defaultdict(list)

        if in_enum:
            brace_depth += line.count("{") - line.count("}")
            if brace_depth <= 0 and in_enum:
                # Leaving the enum body — report any collisions
                for disc, variants in discriminants.items():
                    if len(variants) > 1:
                        locs = ", ".join(f"{vname} (line {ln})" for vname, ln in variants)
                        discriminant_errors.append(
                            f"  DUPLICATE discriminant = {disc} in enum '{enum_name}': {locs}"
                        )
                in_enum = False
                enum_name = ""
                discriminants = defaultdict(list)
                continue

            vm = VARIANT_PATTERN.match(line)
            if vm:
                variant_name = vm.group(1)
                disc_value = int(vm.group(2))
                discriminants[disc_value].append((variant_name, lineno))

    return item_errors, discriminant_errors


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    repo_root = Path(__file__).parent.parent
    print(f"Checking for duplicate Rust items in {repo_root} ...")

    rs_files = sorted(list_rs_files(repo_root))
    print(f"Scanning {len(rs_files)} .rs file(s)\n")

    total_errors = 0

    for path in rs_files:
        rel = path.relative_to(repo_root)
        item_errors, disc_errors = check_file(path)
        all_errors = item_errors + disc_errors
        if all_errors:
            print(f"ERROR in {rel}:")
            for err in all_errors:
                print(err)
            print()
            total_errors += len(all_errors)

    if total_errors:
        print(f"Found {total_errors} duplicate definition(s). Fix them before merging.")
        return 1

    print("OK: no duplicate item definitions found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
