#!/usr/bin/env python3
"""
Detect leftover git merge/conflict artefacts in tracked source files.

Two classes of problem are caught:

1. Standard conflict markers — lines that start with <<<<<<< , >>>>>>> ,
   or consist solely of ======= .  These are normally removed, but the check
   is cheap insurance.

2. Bare branch labels — lines that contain only a branch or ref name such as:
       fix/bump-raffle-ttl-746
       master
       HEAD
   These are left behind when a developer removes the <<< / >>> markers but
   forgets to delete the label lines between them.

File scope
----------
Only files whose extension matches SCANNED_EXTENSIONS are checked.  Markdown
and JSON files are excluded on purpose: branch names appear legitimately in
changelogs and deployment configs.

Exit codes
----------
0 — no artefacts found
1 — at least one artefact found
"""

import re
import subprocess
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Extensions of files that will be scanned.
SCANNED_EXTENSIONS = {".rs", ".ts", ".py", ".sh", ".toml", ".yml", ".yaml"}

# Directories that are never scanned regardless of extension.
SKIP_DIRS = {"target", "fuzz", ".git", "node_modules"}

# ---------------------------------------------------------------------------
# Patterns
# ---------------------------------------------------------------------------

# Standard conflict markers.  These are anchored to the start of the line.
CONFLICT_MARKER_RE = re.compile(r"^(<{7}|>{7}|={7})\s*")

# Bare branch / ref label pattern.
#
# A line is a bare label when — after stripping leading whitespace — it
# matches one of these forms and nothing else:
#   master  main  HEAD  ORIG_HEAD  MERGE_HEAD  REBASE_HEAD
#   fix/<slug>    feat/<slug>    feature/<slug>    release/<slug>
#   chore/<slug>  refactor/<slug>  hotfix/<slug>   bugfix/<slug>
#
# The slug may contain letters, digits, dots, underscores, and hyphens.
# We require at least one slash-containing form or a bare ref keyword so
# plain words never match.
_SLUG = r"[a-zA-Z0-9._-]+"
BARE_LABEL_RE = re.compile(
    r"^\s*"
    r"(?:"
    r"HEAD|ORIG_HEAD|MERGE_HEAD|REBASE_HEAD|master|main"          # bare refs
    r"|(?:fix|feat|feature|release|chore|refactor|hotfix|bugfix)"  # branch prefixes
    r"/" + _SLUG +                                                  # slash + slug
    r")"
    r"\s*$"
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def list_tracked_files(repo_root: Path) -> list[Path]:
    """Return all git-tracked files, falling back to a recursive walk."""
    try:
        result = subprocess.run(
            ["git", "ls-files"],
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

    # Fallback: walk the tree.
    paths = []
    for p in repo_root.rglob("*"):
        if p.is_file() and not any(skip in p.parts for skip in SKIP_DIRS):
            paths.append(p)
    return paths


def should_scan(path: Path) -> bool:
    return path.suffix in SCANNED_EXTENSIONS and not any(
        skip in path.parts for skip in SKIP_DIRS
    )


def check_file(path: Path) -> list[str]:
    """Return a list of error strings for artefacts found in *path*."""
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as exc:
        print(f"  warning: cannot read {path}: {exc}", file=sys.stderr)
        return []

    errors = []
    for lineno, line in enumerate(lines, 1):
        if CONFLICT_MARKER_RE.match(line):
            errors.append(
                f"  line {lineno}: conflict marker: {line.rstrip()!r}"
            )
        elif BARE_LABEL_RE.match(line):
            errors.append(
                f"  line {lineno}: bare branch label: {line.strip()!r}"
            )
    return errors


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    repo_root = Path(__file__).parent.parent
    print(f"Checking for merge/conflict artefacts in {repo_root} ...")

    all_files = list_tracked_files(repo_root)
    scanned = [f for f in all_files if should_scan(f)]
    print(f"Scanning {len(scanned)} file(s) (of {len(all_files)} tracked)\n")

    total_errors = 0

    for path in sorted(scanned):
        rel = path.relative_to(repo_root)
        errors = check_file(path)
        if errors:
            print(f"ERROR in {rel}:")
            for err in errors:
                print(err)
            print()
            total_errors += len(errors)

    if total_errors:
        print(
            f"Found {total_errors} merge artefact(s).\n"
            "Remove conflict markers and bare branch labels before merging."
        )
        return 1

    print("OK: no merge/conflict artefacts found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
