# ADR-001: Event Envelope Format

**Status:** Accepted
**Date:** 2026-07-13
**Deciders:** Joseph O'Brien

## Context

Braid sessions emit a structured event stream covering lifecycle transitions
(`SessionStarted`, `ProviderResponded`, `ToolCalled`, `ToolCompleted`,
`SessionCompleted`). These events must be persisted for replay, audit, and
observability. The format must survive schema evolution without requiring a
migration step or external schema registry.

## Decision

Events are stored as **JSONL** (one JSON object per line) in
`<root>/<session_id>/events.jsonl`. Each line serializes an `Event` struct with
two fields: `session_id` (string) and `kind` (serde-tagged enum).

```json
{"session_id":"s","kind":"SessionStarted"}
{"session_id":"s","kind":{"ToolCalled":{"tool_name":"echo"}}}
```

Companion metadata (`event_count`, `written_at`) is stored in `meta.json`,
written atomically via a `meta.json.tmp` → rename pair so readers never see a
partial file. `meta.json` is absent for in-progress or crash-interrupted
sessions; callers treat its absence as "incomplete" rather than an error.

Unrecognized `kind` values are silently skipped on load — the loader calls
`serde_json::from_str::<Event>` per line and discards failures. An explicit
`EventKind::Unknown { raw }` variant exists for ingester-produced markers; it
is not produced by the loader itself.

A golden test (`event_json_format_is_stable`) pins the exact serialized form
for every current variant. Changing serialization is a deliberate, tested act.

## Consequences

- **Forward-compatible reads**: files written by a newer binary load cleanly in
  an older binary; unknown lines are dropped, not panicked.
- **No schema registry**: variant layout is self-describing JSON; no external
  catalog is needed.
- **Atomic metadata**: `meta.json` presence is a reliable "session complete"
  signal for GC and listing operations.
- **Line-oriented streaming**: `SessionWriter` appends and flushes each event
  immediately; partial sessions are readable mid-run.
- **Trade-off**: unknown future events are silently dropped, not preserved.
  Tooling that requires complete replay across version skips must pin binaries.

## Alternatives Considered

- **Binary/protobuf**: smaller on disk, but requires a schema registry and
  generated code for every consumer.
- **Single JSON array per session**: simpler structure but prevents streaming
  writes and makes partial-session reads impossible without a full parse.
- **Separate payload field**: `{"kind":"ToolCalled","payload":{...}}` envelope.
  Rejected — serde's adjacently tagged enum already provides this structure
  without a wrapper, and the flatter form passes the round-trip test cleanly.
