# braid-observe

`braid-observe` provides Braid's session event storage, incremental writing, replay, rendering, and
normalization of external JSONL transcripts.

## Workspace role and storage format

The crate implements `EventSink` and `SessionStorage` while depending only on domain types and port
traits. The default filesystem layout is one directory per session:

```text
sessions/
└── <session-id>/
    ├── events.jsonl
    └── meta.json
```

`events.jsonl` contains one serialized `braid_model::Event` per line. `meta.json` records the
session ID, RFC 3339 write time, and event count. Metadata is written through a temporary file and
renamed.

## Key APIs

| API | Purpose |
| --- | --- |
| `SessionStore::open` | Create/open a filesystem session root |
| `SessionStore::write/load/list/prune` | Batch persistence and lifecycle operations |
| `SessionStore::load_meta` | Read optional session metadata |
| `SessionWriter` | Append and flush events during an active session |
| `ReplaySession` | Load valid events with one-based indices and raw JSON payloads |
| `render_session` | Produce the stable text timeline consumed by the CLI |
| `Ingester` | Normalize an external JSONL source into a store |
| `BraidIngester` | Ingest native Braid event JSONL |
| `ClaudeCodeIngester` | Normalize selected Claude Code records |
| `DevloopIngester` | Normalize selected DevLoop run records |

## Usage

```rust
use braid_model::{Event, EventKind, SessionId};
use braid_observe::{ReplaySession, SessionStore};

let store = SessionStore::open(std::path::PathBuf::from("./sessions"))?;
let id = SessionId("example".into());
store.write(&id, &[Event {
    session_id: id.clone(),
    kind: EventKind::SessionStarted,
}])?;

let replay = ReplaySession::load(&store, &id)?;
assert_eq!(replay.get(1).map(|event| event.index), Some(1));
# Ok::<(), anyhow::Error>(())
```

`SessionStore` also buffers `EventSink::record` calls and writes them on `flush`. The engine calls
flush explicitly; drop performs a best-effort fallback flush. `SessionWriter` makes each event
visible immediately and writes metadata only from `finish()`.

## Compatibility and safety

- Invalid or future JSONL lines that do not deserialize are skipped so known events remain usable.
- `EventKind::Unknown` round-trips when explicitly serialized.
- Partial sessions without metadata remain loadable and renderable.
- Session listing is newest-first; pruning deletes entries after the newest `keep` sessions.
- The render and event JSON formats are protected by golden/contract tests.

The store does **not** redact automatically. Callers must apply `RedactionPipeline::redact_event`
before `write`, `write_event`, or `record`; audit tests document both safe and unsafe paths.

## Feature flags

| Feature | Effect |
| --- | --- |
| `test-support` | Exposes `InMemorySessionStorage` for tests |

## Implementation status

Filesystem storage, streaming writes, replay, text rendering, three ingesters, forward-compatible
line skipping, and optional in-memory test storage are implemented. Replay currently operates on
the normalized event envelope rather than reconstructing full provider messages or tool payloads.

## Development and testing

```console
cargo nextest run -p braid-observe
cargo nextest run -p braid-observe --features test-support
cargo clippy -p braid-observe -- -D warnings
cargo check -p braid-observe
```

Integration suites cover ingester conformance, stable session formatting, and redact-before-persist
audit scenarios.
