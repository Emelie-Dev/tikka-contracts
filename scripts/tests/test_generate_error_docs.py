"""Unit tests for scripts/generate_error_docs.py.

Tests cover the parsing layer (parse_error_enum) and the rendering helpers
(markdown_table, typescript_mapping) in isolation — no filesystem side-effects.

Edge cases exercised:
  - Happy path: well-formed enum with sequential codes
  - Enum with a gap in discriminant numbers (non-contiguous codes)
  - Duplicate discriminant: same integer assigned to two variants
  - Undocumented variant: name not present in the description/message dicts
  - Enum not found: regex misses the target name
"""

from __future__ import annotations

import sys
import textwrap
from pathlib import Path

import pytest

# Make the scripts package importable without installation.
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from generate_error_docs import (  # noqa: E402
    markdown_table,
    parse_error_enum,
    typescript_mapping,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_enum(tmp_path: Path, enum_name: str, body: str) -> Path:
    """Write a minimal Rust source file containing one enum and return its path."""
    src = tmp_path / "lib.rs"
    src.write_text(
        textwrap.dedent(f"""\
        #[contracterror]
        pub enum {enum_name} {{
        {body}
        }}
        """),
        encoding="utf-8",
    )
    return src


# ---------------------------------------------------------------------------
# parse_error_enum
# ---------------------------------------------------------------------------


class TestParseErrorEnum:
    def test_happy_path_returns_sorted_pairs(self, tmp_path: Path) -> None:
        src = _write_enum(
            tmp_path,
            "Error",
            "    NotFound = 1,\n    Unauthorized = 2,\n    Overflow = 3,",
        )
        result = parse_error_enum(src, "Error")
        assert result == [(1, "NotFound"), (2, "Unauthorized"), (3, "Overflow")]

    def test_gap_in_codes_preserves_all_variants(self, tmp_path: Path) -> None:
        """Discriminants are not required to be contiguous; all must be returned."""
        src = _write_enum(
            tmp_path,
            "Error",
            "    Alpha = 1,\n    Beta = 5,\n    Gamma = 10,",
        )
        result = parse_error_enum(src, "Error")
        assert result == [(1, "Alpha"), (5, "Beta"), (10, "Gamma")]

    def test_duplicate_discriminant_both_variants_returned(self, tmp_path: Path) -> None:
        """parse_error_enum does not deduplicate — callers detect duplicates separately."""
        src = _write_enum(
            tmp_path,
            "Error",
            "    First = 1,\n    Second = 1,\n    Third = 2,",
        )
        result = parse_error_enum(src, "Error")
        # Both discriminant-1 variants must appear so check_duplicates can flag them.
        codes = [code for code, _ in result]
        assert codes.count(1) == 2
        names = [name for _, name in result]
        assert "First" in names
        assert "Second" in names

    def test_output_sorted_by_code(self, tmp_path: Path) -> None:
        """Variants declared out of order are returned sorted by code."""
        src = _write_enum(
            tmp_path,
            "Error",
            "    Zeta = 9,\n    Alpha = 1,\n    Mid = 5,",
        )
        result = parse_error_enum(src, "Error")
        codes = [c for c, _ in result]
        assert codes == sorted(codes)

    def test_enum_not_found_exits_one(self, tmp_path: Path) -> None:
        src = tmp_path / "lib.rs"
        src.write_text("pub enum OtherName { A = 1, }", encoding="utf-8")
        with pytest.raises(SystemExit) as exc_info:
            parse_error_enum(src, "Error")
        assert exc_info.value.code == 1

    def test_ignores_variants_without_discriminant(self, tmp_path: Path) -> None:
        """Fieldless variants without an explicit `= N` assignment are skipped."""
        src = _write_enum(
            tmp_path,
            "Error",
            "    Valid = 1,\n    NoCode,\n    AlsoValid = 2,",
        )
        result = parse_error_enum(src, "Error")
        names = [n for _, n in result]
        assert "NoCode" not in names
        assert "Valid" in names
        assert "AlsoValid" in names


# ---------------------------------------------------------------------------
# markdown_table
# ---------------------------------------------------------------------------


class TestMarkdownTable:
    def test_columns_present(self) -> None:
        errors = [(1, "NotFound"), (2, "Overflow")]
        descs = {"NotFound": "desc A", "Overflow": "desc B"}
        msgs = {"NotFound": "msg A", "Overflow": "msg B"}
        table = markdown_table(errors, descs, msgs)
        assert "| Code |" in table
        assert "| Error |" in table
        assert "| Description |" in table
        assert "| Frontend Message |" in table

    def test_row_values_appear(self) -> None:
        errors = [(42, "MyError")]
        descs = {"MyError": "Something went wrong"}
        msgs = {"MyError": "Oops"}
        table = markdown_table(errors, descs, msgs)
        assert "42" in table
        assert "`MyError`" in table
        assert "Something went wrong" in table
        assert '"Oops"' in table

    def test_undocumented_variant_uses_todo_placeholder(self) -> None:
        """A variant absent from both dicts gets the TODO placeholder, not a crash."""
        errors = [(99, "UnknownVariant")]
        table = markdown_table(errors, {}, {})
        assert "TODO: Add description" in table
        assert "TODO: Add message" in table

    def test_rows_ordered_by_input_sequence(self) -> None:
        """Rows follow the order of the input list (callers sort by code)."""
        errors = [(1, "A"), (5, "B"), (10, "C")]
        descs = {"A": "a", "B": "b", "C": "c"}
        msgs = {"A": "a", "B": "b", "C": "c"}
        table = markdown_table(errors, descs, msgs)
        pos_a = table.index("| 1 |")
        pos_b = table.index("| 5 |")
        pos_c = table.index("| 10 |")
        assert pos_a < pos_b < pos_c


# ---------------------------------------------------------------------------
# typescript_mapping
# ---------------------------------------------------------------------------


class TestTypescriptMapping:
    def test_produces_valid_ts_block(self) -> None:
        errors = [(1, "NotFound"), (2, "Overflow")]
        msgs = {"NotFound": "not found msg", "Overflow": "overflow msg"}
        # Temporarily patch the module-level dict used inside the function.
        import generate_error_docs as mod

        original = mod.INSTANCE_MESSAGES
        mod.INSTANCE_MESSAGES = msgs
        try:
            result = typescript_mapping(errors)
        finally:
            mod.INSTANCE_MESSAGES = original

        assert "```typescript" in result
        assert "Record<number, string>" in result
        assert "1:" in result
        assert "2:" in result
        assert '"not found msg"' in result

    def test_missing_variant_falls_back_to_todo(self) -> None:
        import generate_error_docs as mod

        original = mod.INSTANCE_MESSAGES
        mod.INSTANCE_MESSAGES = {}
        try:
            result = typescript_mapping([(7, "Ghost")])
        finally:
            mod.INSTANCE_MESSAGES = original

        assert "TODO: Add message" in result
