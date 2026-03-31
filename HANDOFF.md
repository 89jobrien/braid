# Braid Handoff Notes

## 2026-03-30 — P2: SessionWriter Durability + Dependency Lint

### What changed

**Commit `9f1d9a1`** — `test(braid-observe): add P2 SessionWriter durability tests + remove unused thiserror deps`

#### SessionWriter durability tests (3 new)

- `drop_without_finish_leaves_events_readable` — events survive unclean shutdown; meta.json absent signals incomplete session
- `partial_write_last_line_is_skipped` — truncated final line from mid-write crash is skipped; prior events intact
- `finish_is_atomic_no_tmp_left_behind` — atomic rename leaves no `.tmp` file visible to concurrent readers

#### Dependency lint

Removed unused `thiserror` from `braid-model` and `braid-observe`. Neither crate defines custom error types (both use `anyhow`). `cargo-machete` now clean.

### Test count

Before: 128 tests | After: 132 tests (+4)

### Remaining backlog

None. All P1 and P2 items from council analysis are resolved.

---

## 2026-03-30 — Council Analysis + P1 Test Coverage

### What changed

Council analysis (devloop analyze) ran on the `main` branch (health score: 74%).
No P0 bugs were found. All P1 findings were test coverage gaps in the new
`braid-observe` streaming pipeline introduced over the past week.

**Commit `aa0f029`** — `test(braid-observe): add P1 coverage for streaming pipeline and ingester conformance`

Three test suites added to `crates/braid-observe/`:

#### 1. `src/e2e_streaming_tests.rs` — End-to-end streaming persistence + replay

Tests that the full `Engine → with_event_callback → SessionWriter → SessionStore → ReplaySession`
pipeline preserves events in order with no data loss:

- `simple_session_events_persisted_and_replayed_in_order` — single-turn, 3-event session
- `tool_call_events_persisted_with_correct_tool_name` — tool round-trip, 6-event session, payload losslessness
- `engine_events_match_replayed_events_losslessly` — position-by-position comparison of `RunOutput.events` vs `ReplaySession`

#### 2. `src/ingest.rs` — `EventKind::Unknown` forward-compat round-trip

Test that `EventKind::Unknown { raw }` survives the full ingestion pipeline:
BraidIngester → store.load() → ReplaySession, without dropping the `raw` field.

- `unknown_event_kind_round_trips_through_ingestion_and_replay`

#### 3. `tests/conformance.rs` — Ingester conformance suite

Tests that `BraidIngester`, `DevloopIngester`, and `ClaudeCodeIngester` all
produce sessions with consistent shape (non-empty event lists, valid session IDs,
typed event kinds):

- `braid_ingester_conformance`
- `devloop_ingester_conformance`
- `claude_code_ingester_conformance`
- `all_ingesters_produce_consistent_event_shape`

### Test count

Before: 119 tests | After: 127 tests (+8)

### Remaining P2 items (not addressed)

- SessionWriter durability/flush tests (partial writes, finalization on shutdown)
- Crate boundary documentation + lightweight dependency lint

### Council findings (full categorization)

| Priority | Finding | Status |
|---|---|---|
| P1 | E2E: Engine → SessionWriter → replay losslessness | ✅ Fixed |
| P1 | EventKind::Unknown round-trip through ingestion | ✅ Fixed |
| P1 | Ingester conformance suite | ✅ Fixed |
| P2 | SessionWriter durability/flush semantics | Backlog |
| P2 | Crate boundary docs + dependency lint | Backlog |
