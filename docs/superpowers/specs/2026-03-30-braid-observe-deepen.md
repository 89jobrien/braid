---
type: design
topic: [braid-observe, braid-tui, ingest, normalize, replay, tui]
status: approved
tags: [rust, observe, tui, ingest, phase3]
---

# braid-observe Deepen + braid-tui Design

## Goal

Deepen `braid-observe` with a streaming ingest pipeline, multi-source normalization, and a
replay layer — then build `braid-tui`, a standalone ratatui-based multi-pane session inspector
that drives the design.

## Approach

TUI-first: design from what the inspector needs to see, then work backwards to what the data
layer must expose. Implementation is two sequential phases:

- **Phase 3a** — `braid-observe` additions: streaming writer, ingest adapters, normalize, replay
- **Phase 3b** — `braid-tui`: standalone ratatui binary, three-pane inspector

---

## Phase 3a: braid-observe Data Layer

### Streaming Ingest — `SessionWriter`

Replace the current all-at-once `store.write()` call in `cmd_run` with a `SessionWriter` that
opens a session directory before the engine loop and flushes each event immediately on
`write_event()`. Partial sessions survive crashes. On drop, writes `meta.json` atomically.

```rust
pub struct SessionWriter { /* internal */ }

impl SessionWriter {
    pub fn open(root: &Path, id: &SessionId) -> Result<Self>;
    pub fn write_event(&mut self, event: &Event) -> Result<()>;
    // Finalizes meta.json on drop (also callable explicitly)
    pub fn finish(self) -> Result<()>;
}
```

`Engine` gains an optional event callback: `.with_event_callback(fn(&Event))`. `cmd_run` in
`braid-cli` creates a `SessionWriter` before calling `engine.run()`, registers a callback that
calls `writer.write_event()` on each emitted event, and calls `writer.finish()` after the run.
Events are redacted before being passed to the callback.

### Multi-Source Ingest — `Ingester` Trait

```rust
pub trait Ingester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId>;
}
```

Three implementations:

| Adapter | Source format | Notes |
|---|---|---|
| `BraidIngester` | braid-native JSONL | Copies lines verbatim; validates each parses as `Event` |
| `ClaudeCodeIngester` | `.claude/projects/*/conversations/*.jsonl` | Maps Claude Code message roles/content to braid `Event`s |
| `DevloopIngester` | devloop run transcript JSONL | Maps devloop event schema to braid `Event`s |

Unknown fields are dropped. Unknown event kinds normalize to `EventKind::Unknown { raw: String }`
for forward-compatibility. Each adapter is independently testable with fixture files.

### Normalization — `EventKind::Unknown`

Add to `braid-model`:

```rust
// In EventKind enum:
Unknown { raw: String },
```

`render_session` and the TUI render `Unknown` rows with the raw string as the detail payload.
`store.load()` already skips unparseable lines (forward-compat); `Unknown` handles the case
where the line parses but the kind is unrecognized.

### Replay Layer

```rust
pub struct ReplaySession {
    pub id: SessionId,
    events: Vec<ReplayEvent>,
}

pub struct ReplayEvent {
    pub index: usize,       // 1-based
    pub event: Event,
    pub payload: Option<serde_json::Value>, // raw JSON value from JSONL line
}

impl ReplaySession {
    pub fn load(store: &SessionStore, id: &SessionId) -> Result<Self>;
    pub fn iter(&self) -> impl Iterator<Item = &ReplayEvent>;
    pub fn get(&self, index: usize) -> Option<&ReplayEvent>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

Payload is the raw JSON value from the JSONL line, preserved verbatim. Re-execution and
stepping are future extensions — they will implement a `ReplayExecutor` trait against this same
type without breaking the current API.

### Public API additions to `braid-observe`

```rust
// lib.rs additions
pub mod ingest;
pub mod replay;

pub use ingest::{BraidIngester, ClaudeCodeIngester, DevloopIngester, Ingester};
pub use replay::{ReplayEvent, ReplaySession};
pub use store::SessionWriter; // added to existing store module
```

---

## Phase 3b: braid-tui

### Crate

Standalone binary crate `crates/braid-tui`. Dependencies: `ratatui`, `crossterm`,
`braid-observe`, `braid-model`, `anyhow`. No dependency on `braid-cli`. Binary name: `braid-tui`.

Entry point: `braid-tui` (run directly). A `braid inspect` alias in `braid-cli` can be added
later.

### Layout

```
┌─────────────────────────────────────────────────────┐
│  braid inspect                              q: quit  │
├──────────────┬──────────────────────────────────────┤
│ Sessions     │ Timeline           1748293847         │
│──────────────│──────────────────────────────────────│
│ 1748293847 ◀ │   1  SessionStarted                  │
│ 1748293821   │   2  ProviderResponded                │
│ 1748293800   │   3  ToolCalled          echo       ▶ │
│ 1748293755   │   4  ToolCompleted       echo         │
│              │   5  SessionCompleted                 │
│              ├──────────────────────────────────────┤
│              │ Detail  (expanded)                   │
│              │  {"tool_name": "echo",               │
│              │   "input": "hello world"}            │
└──────────────┴──────────────────────────────────────┘
```

- Session list: ~25% width, left pane
- Timeline: ~75% width, upper-right; fills full right column when detail is collapsed
- Detail: lower-right, ~30% of right column height when expanded; hidden when collapsed
- Status bar: single line at top showing app name and key hints

### State Machine

```rust
enum Focus { SessionList, Timeline }

enum DetailState { Collapsed, Expanded(usize) } // usize = ReplayEvent index

struct AppState {
    sessions: Vec<SessionId>,       // newest first
    selected_session: usize,        // index into sessions
    loaded: Option<ReplaySession>,  // None if load failed or empty store
    focus: Focus,
    timeline_cursor: usize,         // index into loaded.events (0-based)
    detail: DetailState,
    store: SessionStore,
}
```

### Key Bindings

| Key | Action |
|---|---|
| `Tab` | Cycle focus: SessionList ↔ Timeline |
| `↑` / `↓` | Navigate within focused pane |
| `Enter` | Toggle detail expand/collapse on focused timeline row |
| `q` | Quit |
| `r` | Reload current session from disk |

### Edge Cases

| Situation | Behavior |
|---|---|
| Empty store | "no sessions" in list pane; timeline shows placeholder |
| Session load failure | Error message in timeline pane; session remains selected |
| `Unknown` event kind | Renders as `Unknown` row; raw JSON is the detail payload |
| Terminal < 80×24 | Single-line "terminal too small (need 80×24)" message |
| Crash | Read-only — no writes, no data loss risk |

---

## File Map

### Phase 3a — braid-observe additions

| File | Change |
|---|---|
| `crates/braid-model/src/event.rs` | Add `EventKind::Unknown { raw: String }` |
| `crates/braid-observe/src/store.rs` | Add `SessionWriter` struct and impl |
| `crates/braid-observe/src/ingest.rs` | New: `Ingester` trait + three adapters |
| `crates/braid-observe/src/replay.rs` | New: `ReplaySession`, `ReplayEvent` |
| `crates/braid-observe/src/lib.rs` | Re-export new modules |
| `crates/braid-cli/src/main.rs` | Wire `SessionWriter` into `cmd_run` |

### Phase 3b — braid-tui

| File | Change |
|---|---|
| `crates/braid-tui/Cargo.toml` | New crate manifest |
| `crates/braid-tui/src/main.rs` | Entry point, terminal setup/teardown |
| `crates/braid-tui/src/app.rs` | `AppState`, event loop |
| `crates/braid-tui/src/ui.rs` | ratatui render functions |
| `crates/braid-tui/src/keys.rs` | Key binding handler → state transitions |
| `Cargo.toml` (root) | Add `braid-tui` to workspace members |

---

## Testing Strategy

### Phase 3a

- `SessionWriter`: write events incrementally via writer, load via `store.load()`, compare — same
  roundtrip guarantee as current batch write tests
- Each ingest adapter: fixture JSON file → `ingest()` → assert expected `Vec<Event>`. Fixtures
  live in `crates/braid-observe/fixtures/`
- `ReplaySession::load`: loads a known session, asserts index, event kind, payload fields
- `EventKind::Unknown`: roundtrip test that an unrecognized kind survives as `Unknown { raw }`

### Phase 3b

- No widget rendering tests (ratatui output is hard to assert)
- Pure state machine tests: construct `AppState`, send key events, assert resulting state
  (focus, cursor position, detail state, selected session index)
- One smoke test: open store with fixture sessions, construct app, verify initial state is
  most-recent session selected with timeline loaded

---

## Non-Goals

- Re-execution of sessions (future `ReplayExecutor` — architecture supports it, not built here)
- Step-through debugger mode (future)
- `braid inspect` alias in `braid-cli` (future)
- Search / filter within the TUI (future)
- Mouse support (future)
