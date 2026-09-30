#!/usr/bin/env python3
"""CI check: ensure no duplicate or reused discriminants within each Rust error enum.

Usage:
    python scripts/check_error_codes.py

Exits non-zero if a duplicate discriminant is found within any single enum.
"""
"""Check live contract errors against the canonical ProtocolError catalog."""

import re
import sys
from pathlib import Path


def parse_error_enum(file_path: Path, enum_name: str) -> list[tuple[int, str]]:
    content = file_path.read_text(encoding="utf-8")

    enum_match = re.search(
        r"(?:#\[contracterror\].*?)?pub enum " + enum_name + r" \{(.*?)\}",
def parse_error_enum(file_path, enum_name):
    content = Path(file_path).read_text(encoding="utf-8")
    enum_match = re.search(
        r"pub enum " + re.escape(enum_name) + r"\s*\{(.*?)\}",
        content,
        re.DOTALL,
    )

    if not enum_match:
        return []

    enum_body = enum_match.group(1)
    errors = []
    for match in re.finditer(r"(\w+)\s*=\s*(\d+)", enum_body):
        errors.append((int(match.group(2)), match.group(1)))

    return errors


def check_duplicates(errors: list[tuple[int, str]], enum_name: str) -> bool:
    seen: dict[int, str] = {}
    for code, name in errors:
        if code in seen:
            print(
                f"ERROR: Duplicate discriminant {code} in {enum_name}: "
                f"{seen[code]} and {name}"
            )
            return False
        seen[code] = name
    return True
    if not enum_match:
        raise ValueError(f"Could not find enum {enum_name} in {file_path}")

    errors = []
    variant_pattern = re.compile(
        r"^[ \t]*(\w+)[ \t]*=[ \t]*(\d+),?[ \t]*"
        r"(?://[ \t]*factory original:[ \t]*(\d+))?[ \t]*$",
        re.MULTILINE,
    )
    for match in variant_pattern.finditer(enum_match.group(1)):
        name = match.group(1)
        code = int(match.group(2))
        original_factory_code = match.group(3)
        errors.append((code, name, int(original_factory_code) if original_factory_code else None))
    return errors


def check_duplicates(errors, enum_name):
    seen_codes = {}
    seen_names = set()
    ok = True
    for code, name, _ in errors:
        if name in seen_names:
            print(f"ERROR: Duplicate variant {name} in {enum_name}")
            ok = False
        seen_names.add(name)
        if code in seen_codes:
            print(f"ERROR: Duplicate discriminant {code} in {enum_name}: "
                  f"{seen_codes[code]} and {name}")
            ok = False
        seen_codes[code] = name
    return ok


def check_catalog(live_errors, catalog_errors, enum_name, factory=False):
    # Factory ABI codes overlap instance codes, so compare their recorded ABI
    # values while keeping the catalog's factory IDs in the 200+ namespace.
    catalog_by_name = {name: (code, original_code) for code, name, original_code in catalog_errors}
    ok = True
    for live_code, live_name, _ in live_errors:
        catalog_name = f"Factory{live_name}" if factory else live_name
        catalog_entry = catalog_by_name.get(catalog_name)
        if catalog_entry is None:
            print(f"ERROR: {enum_name}::{live_name} is missing from ProtocolError as {catalog_name}")
            ok = False
            continue

        catalog_code, original_code = catalog_entry
        if factory:
            if not 200 <= catalog_code <= 299:
                print(f"ERROR: ProtocolError::{catalog_name} must use a factory catalog code in 200–299")
                ok = False
            if original_code != live_code:
                print(f"ERROR: ProtocolError::{catalog_name} maps to factory ABI code "
                      f"{original_code}, but {enum_name}::{live_name} uses {live_code}")
                ok = False
        elif catalog_code != live_code:
            print(f"ERROR: ProtocolError::{catalog_name} uses {catalog_code}, "
                  f"but {enum_name}::{live_name} uses {live_code}")
            ok = False
    return ok


def main() -> None:
    repo_root = Path(__file__).parent.parent
    instance_file = repo_root / "contracts" / "raffle-instance" / "src" / "lib.rs"
    factory_file = repo_root / "contracts" / "raffle-factory" / "src" / "lib.rs"

    instance_errors = parse_error_enum(instance_file, "Error")
    factory_errors = parse_error_enum(factory_file, "ContractError")

    ok = check_duplicates(instance_errors, "raffle-instance::Error")
    ok = check_duplicates(factory_errors, "raffle-factory::ContractError") and ok
    catalog_file = repo_root / "contracts" / "raffle-shared" / "src" / "errors.rs"

    try:
        instance_errors = parse_error_enum(instance_file, "Error")
        factory_errors = parse_error_enum(factory_file, "ContractError")
        catalog_errors = parse_error_enum(catalog_file, "ProtocolError")
    except ValueError as error:
        print(f"ERROR: {error}")
        sys.exit(1)

    ok = True
    ok &= check_duplicates(instance_errors, "raffle-instance::Error")
    ok &= check_duplicates(factory_errors, "raffle-factory::ContractError")
    ok &= check_duplicates(catalog_errors, "raffle-shared::ProtocolError")
    ok &= check_catalog(instance_errors, catalog_errors, "raffle-instance::Error")
    ok &= check_catalog(factory_errors, catalog_errors, "raffle-factory::ContractError", factory=True)

    if not ok:
        print("\nCI check FAILED: error definitions and ProtocolError disagree.")
        sys.exit(1)

    print("CI check PASSED: no duplicate discriminants.")
    print("CI check PASSED: live error enums match ProtocolError.")
    sys.exit(0)


if __name__ == "__main__":
    main()
