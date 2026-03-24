# braid-observe Design

**Date:** 2026-03-24
**Status:** Approved
**Phase:** 3

---

## Overview

`braid-observe` is the observability crate for braid. It persists engine session events to disk and provides a human-readable inspector for completed runs. It is the first Phase 3 crate.

**Definition of done:** a completed run can be inspected from stored events. "Replay" in this spec means read-back and print (inspection), not re-execution. This satisfies the Phase 3 checklist item "inspected and replayed" at its intended scope.

**Hard boundary:** `braid-observe` does NOT redact. It receives already-redacted events from the caller. It depends on `braid-model` only — not `braid-redact`. Redact-before-persist is enforced at the call site (CLI).

---

## Crate Layout

```
crates/braid-observe/
  Cargo.toml
  src/
    lib.rs       # pub re-exports
    store.rs     # SessionStore
    render.rs    # render_session()
```

Dependencies: `braid-model`, `anyhow`, `serde`, `serde_json`, `thiserror` (all workspace). No new external crates. Dev dependency: `tempfile`.

---

## Storage Layout

```
<root>/                        # default: ~/.braid/sessions/
  <session-id>/
    events.jsonl               # one JSON-serialized Event per line
    meta.json                  # session metadata
```

Subdirectory per session (not flat files) so future additions — artifacts, context snapshots — have a natural home without a format change.

### events.jsonl

One `Event` per line using existing serde derives on `braid_model::Event`:

```jsonl
{"session_id":"abc","kind":"SessionStarted"}
{"session_id":"abc","kind":"ProviderResponded"}
{"session_id":"abc","kind":{"ToolCalled":{"tool_name":"echo"}}}
{"session_id":"abc","kind":{"ToolCompleted":{"tool_name":"echo"}}}
{"session_id":"abc","kind":"SessionCompleted"}
```

### meta.json

Written atomically alongside events:

```json
{ "session_id": "abc", "written_at": "2026-03-24T05:00:00Z", "event_count": 5 }
```

`list()` sorts sessions by `written_at` from meta (falling back to directory mtime). `prune()` deletes the oldest directories beyond the keep threshold. `prune()` must delegate to `list()` internally to guarantee consistent ordering — it must not independently re-implement the sort key.

---

## Public API

```rust
/// Metadata written alongside a session's events.
pub struct SessionMeta {
    pub session_id: SessionId,
    pub written_at: String,   // RFC 3339
    pub event_count: usize,
}

/// Manages a root directory of persisted sessions.
pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    /// Open (or create) a store at the given root directory.
    pub fn open(root: PathBuf) -> Result<Self>;

    /// Write events for a session. Caller must pre-redact events.
    /// Writes events.jsonl first, then meta.json atomically (write-then-rename).
    pub fn write(&self, id: &SessionId, events: &[Event]) -> Result<()>;

    /// List all session IDs, newest first.
    pub fn list(&self) -> Result<Vec<SessionId>>;

    /// Load events for a session.
    /// If meta.json is absent or event_count mismatches, still returns events from events.jsonl (best-effort).
    /// Returns Err only if events.jsonl is absent or unreadable.
    pub fn load(&self, id: &SessionId) -> Result<Vec<Event>>;

    /// Load metadata for a session. Returns Ok(None) if meta.json is absent.
    pub fn load_meta(&self, id: &SessionId) -> Result<Option<SessionMeta>>;

    /// Delete oldest sessions, keeping `keep` most recent.
    /// Returns the number of sessions deleted.
    pub fn prune(&self, keep: usize) -> Result<usize>;
}
```

---

## Render Format

`render.rs` exposes one public function:

```rust
pub fn render_session(
    events: &[Event],
    meta: Option<&SessionMeta>,
    out: &mut impl Write,
) -> Result<()>
```

Output format (plain ASCII, no color, works in piped output):

```
Session: abc  (2026-03-24 05:00:00 UTC)  5 events
--------------------------------------------------
  1  SessionStarted
  2  ProviderResponded
  3  ToolCalled          echo
  4  ToolCompleted       echo
  5  SessionCompleted
```

Rules:
- Fixed-width index column; event kind left-aligned; optional detail (e.g. tool name) right
- Separator drawn to terminal width, capped at 72 characters
- `written_at` from `meta.json`; omitted gracefully if meta absent
- No unicode box-drawing, no external formatting crates

---

## CLI Integration

### Wiring into `cmd_run`

After `engine.run()`, the CLI persists events (non-fatal on failure):

```rust
let store = SessionStore::open(default_store_dir()?)?;
// Construct a second pipeline for event redaction — the first is moved into the engine closure.
let event_pipeline = RedactionPipeline::new()
    .with_rule(SecretPatternRule::new())
    .with_rule(EnvVarRule::new())
    .with_rule(HomePathRule::new());
let redacted_events: Vec<Event> = output.events.iter()
    .map(|e| event_pipeline.redact_event(e))
    .collect();
if let Err(e) = store.write(&session_id, &redacted_events) {
    eprintln!("warn: could not persist session: {e}");
}
```

`default_store_dir()` returns `$HOME/.braid/sessions/`.

**Note on `redact_event()`:** Events carry tool names and session IDs, not message content — `redact_event()` redacts the `tool_name` field in `ToolCalled`/`ToolCompleted` events. Message content never appears in `Event`; it lives only in the `messages` conversation history, which is redacted by the engine's `with_redactor()` before reaching the provider.

**Note on `SessionId`:** `cmd_run` must generate a unique `SessionId` per invocation (e.g. a timestamp-based ID). The current hardcoded `SessionId("session".into())` must be replaced before sessions are persisted, or each run will overwrite the previous one.

### New subcommands

```
braid sessions list              # print session IDs, newest first
braid sessions show <id>         # print a session's event timeline
braid sessions prune [--keep N]  # delete oldest, keep N (default: 50)
```

### Cargo.toml changes

- `braid-cli/Cargo.toml` gains `braid-observe = { path = "../braid-observe" }`
- `braid-observe/Cargo.toml` lists only `braid-model` + workspace deps

---

## Testing

All store tests use a real temporary directory via `tempfile::tempdir()`.

| Test | What it checks |
|---|---|
| `writes_and_loads_roundtrip` | write events → load → assert equal |
| `list_returns_sessions_by_recency` | write 3 sessions → list → newest first |
| `prune_removes_oldest` | write 5 → prune keep=3 → 3 remain |
| `load_missing_session_errors` | load nonexistent id → `Err` |
| `renders_all_event_kinds` | snapshot of formatted output covers all `EventKind` variants |
| `renders_gracefully_without_meta` | meta absent → no crash, no timestamp line |
| `load_succeeds_when_meta_absent` | write events.jsonl manually without meta.json → load returns events |

---

## What This Is Not

- **Not a query engine.** No SQL, no FTS. Raw JSONL + directory listing only.
- **Not a redaction layer.** Events must be redacted before calling `write()`.
- **Not live/streaming.** Events are written post-run as a batch.
- **Not a TUI.** Plain stdout rendering only. A TUI viewer can be added later.
