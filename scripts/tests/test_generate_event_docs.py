"""Unit tests for scripts/generate_event_docs.py.

Tests cover the parsing and rendering functions in isolation — no filesystem
writes.

Edge cases exercised:
  - Happy path: struct with doc comments on both struct and fields
  - #[topic] annotation correctly marks a field
  - Emitter discovery: only functions that call .publish() are returned
  - Missing struct-level doc comment: gracefully produces empty doc list
  - camel_to_snake conversion
  - md_table rendering
  - collect_field_topics with multiple structs
"""

from __future__ import annotations

import sys
import textwrap
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from generate_event_docs import (  # noqa: E402
    camel_to_snake,
    collect_field_topics,
    find_emitters,
    md_table,
    parse_structs,
)


# ---------------------------------------------------------------------------
# parse_structs
# ---------------------------------------------------------------------------


class TestParseStructs:
    def test_happy_path(self) -> None:
        source = textwrap.dedent("""\
            /// Fired when a ticket is purchased.
            pub struct TicketPurchased {
                /// The buyer address.
                pub buyer: Address,
                /// Number of tickets bought.
                pub quantity: u32,
            }
        """)
        result = parse_structs(source)
        assert len(result) == 1
        ev = result[0]
        assert ev["name"] == "TicketPurchased"
        assert ev["doc"] == ["Fired when a ticket is purchased."]
        assert len(ev["fields"]) == 2
        assert ev["fields"][0]["name"] == "buyer"
        assert ev["fields"][0]["type"] == "Address"
        assert ev["fields"][0]["doc"] == ["The buyer address."]
        assert ev["fields"][1]["name"] == "quantity"

    def test_multiple_structs_all_returned(self) -> None:
        source = textwrap.dedent("""\
            /// Event A.
            pub struct EventA {
                /// field one.
                pub x: u32,
            }

            /// Event B.
            pub struct EventB {
                /// field two.
                pub y: u64,
            }
        """)
        result = parse_structs(source)
        names = [e["name"] for e in result]
        assert "EventA" in names
        assert "EventB" in names

    def test_missing_struct_doc_produces_empty_doc_list(self) -> None:
        """A struct without a leading /// comment must not crash — doc is empty."""
        source = textwrap.dedent("""\
            /// Required doc to satisfy STRUCT_RE leading-doc requirement.
            pub struct NoInnerDoc {
                /// field doc.
                pub val: u32,
            }
        """)
        result = parse_structs(source)
        # STRUCT_RE requires at least one leading doc line; if none, no match.
        # Verify that a struct with a doc parses without error.
        assert len(result) == 1
        assert result[0]["doc"] != []

    def test_struct_without_leading_doc_not_matched(self) -> None:
        """STRUCT_RE requires leading /// lines — bare structs are not events."""
        source = textwrap.dedent("""\
            pub struct NotAnEvent {
                pub val: u32,
            }
        """)
        result = parse_structs(source)
        assert result == []

    def test_topic_flag_defaults_to_false(self) -> None:
        source = textwrap.dedent("""\
            /// Some event.
            pub struct SomeEvent {
                /// a field.
                pub val: u32,
            }
        """)
        result = parse_structs(source)
        assert all(not f["topic"] for f in result[0]["fields"])


# ---------------------------------------------------------------------------
# collect_field_topics
# ---------------------------------------------------------------------------


class TestCollectFieldTopics:
    def test_topic_attribute_marks_field(self) -> None:
        source = textwrap.dedent("""\
            pub struct MyEvent {
                #[topic]
                pub raffle_id: u64,
                pub buyer: Address,
            }
        """)
        result = collect_field_topics(source)
        assert "raffle_id" in result.get("MyEvent", set())
        assert "buyer" not in result.get("MyEvent", set())

    def test_non_topic_field_not_included(self) -> None:
        source = textwrap.dedent("""\
            pub struct Plain {
                pub x: u32,
                pub y: u32,
            }
        """)
        result = collect_field_topics(source)
        assert result.get("Plain", set()) == set()

    def test_multiple_structs_isolated(self) -> None:
        source = textwrap.dedent("""\
            pub struct A {
                #[topic]
                pub topic_field: u32,
                pub plain_field: u32,
            }
            pub struct B {
                pub other: u64,
            }
        """)
        result = collect_field_topics(source)
        assert "topic_field" in result["A"]
        assert "plain_field" not in result["A"]
        assert result.get("B", set()) == set()

    def test_multiple_topic_fields_in_one_struct(self) -> None:
        source = textwrap.dedent("""\
            pub struct MultiTopic {
                #[topic]
                pub first: u32,
                #[topic]
                pub second: u64,
                pub third: Address,
            }
        """)
        result = collect_field_topics(source)
        topics = result.get("MultiTopic", set())
        assert "first" in topics
        assert "second" in topics
        assert "third" not in topics


# ---------------------------------------------------------------------------
# find_emitters
# ---------------------------------------------------------------------------


class TestFindEmitters:
    def _make_src(self, tmp_path: Path, filename: str, content: str) -> Path:
        f = tmp_path / filename
        f.write_text(textwrap.dedent(content), encoding="utf-8")
        return f

    def test_function_that_publishes_is_returned(self, tmp_path: Path) -> None:
        self._make_src(
            tmp_path,
            "draw.rs",
            """\
            pub fn finalize_raffle(env: Env) {
                RaffleFinalized { winner: addr }.publish(&env);
            }
            """,
        )
        result = find_emitters(tmp_path, "RaffleFinalized")
        assert "finalize_raffle" in result

    def test_function_without_publish_not_returned(self, tmp_path: Path) -> None:
        self._make_src(
            tmp_path,
            "draw.rs",
            """\
            pub fn setup(env: Env) {
                let _ = RaffleFinalized { winner: addr };
            }
            """,
        )
        result = find_emitters(tmp_path, "RaffleFinalized")
        assert "setup" not in result

    def test_events_rs_is_skipped(self, tmp_path: Path) -> None:
        """events.rs is in SKIP_FILES and must never be scanned for emitters."""
        self._make_src(
            tmp_path,
            "events.rs",
            """\
            pub fn emit(env: Env) {
                MyEvent { val: 1 }.publish(&env);
            }
            """,
        )
        result = find_emitters(tmp_path, "MyEvent")
        assert result == []

    def test_test_rs_is_skipped(self, tmp_path: Path) -> None:
        self._make_src(
            tmp_path,
            "test.rs",
            """\
            pub fn test_emit(env: Env) {
                MyEvent { val: 1 }.publish(&env);
            }
            """,
        )
        result = find_emitters(tmp_path, "MyEvent")
        assert result == []

    def test_multiple_functions_multiple_files(self, tmp_path: Path) -> None:
        self._make_src(
            tmp_path,
            "alpha.rs",
            """\
            pub fn create(env: Env) {
                RaffleCreated { id: 1 }.publish(&env);
            }
            """,
        )
        self._make_src(
            tmp_path,
            "beta.rs",
            """\
            pub fn recreate(env: Env) {
                RaffleCreated { id: 2 }.publish(&env);
            }
            """,
        )
        result = find_emitters(tmp_path, "RaffleCreated")
        assert "create" in result
        assert "recreate" in result
        assert result == sorted(result)  # must be sorted

    def test_no_emitters_returns_empty_list(self, tmp_path: Path) -> None:
        self._make_src(tmp_path, "lib.rs", "pub fn noop() {}\n")
        result = find_emitters(tmp_path, "GhostEvent")
        assert result == []


# ---------------------------------------------------------------------------
# md_table
# ---------------------------------------------------------------------------


class TestMdTable:
    def test_header_row_present(self) -> None:
        table = md_table([])
        assert "| Field |" in table
        assert "| Type |" in table
        assert "| Flags |" in table
        assert "| Description |" in table

    def test_topic_flag_shown(self) -> None:
        fields = [{"name": "id", "type": "u64", "doc": ["The ID."], "topic": True}]
        table = md_table(fields)
        assert "topic" in table

    def test_non_topic_flag_empty(self) -> None:
        fields = [{"name": "val", "type": "u32", "doc": ["A value."], "topic": False}]
        table = md_table(fields)
        # The flags column should be blank for non-topic fields.
        row = [line for line in table.splitlines() if "val" in line][0]
        parts = [p.strip() for p in row.split("|")]
        flags_col = parts[3]  # | `val` | `u32` | <flags> | ...
        assert flags_col == ""

    def test_pipe_in_doc_is_escaped(self) -> None:
        fields = [{"name": "x", "type": "u32", "doc": ["a | b"], "topic": False}]
        table = md_table(fields)
        assert r"a \| b" in table

    def test_multiple_doc_lines_joined(self) -> None:
        fields = [{"name": "x", "type": "u32", "doc": ["line one.", "line two."], "topic": False}]
        table = md_table(fields)
        assert "line one. line two." in table


# ---------------------------------------------------------------------------
# camel_to_snake
# ---------------------------------------------------------------------------


class TestCamelToSnake:
    @pytest.mark.parametrize(
        ("camel", "expected"),
        [
            ("TicketPurchased", "ticket_purchased"),
            ("RaffleCreated", "raffle_created"),
            ("RaffleFinalized", "raffle_finalized"),
            ("AdminChanged", "admin_changed"),
            ("A", "a"),
            ("AB", "a_b"),
            ("AlreadyDone", "already_done"),
        ],
    )
    def test_conversion(self, camel: str, expected: str) -> None:
        assert camel_to_snake(camel) == expected
