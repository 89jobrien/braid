# Session Persistence

`braid-observe` handles everything related to session storage: writing, reading, rendering, ingesting external formats, and replaying for inspection.

## On-Disk Layout

```
~/.braid/sessions/
├── 1743300000/
│   ├── events.jsonl     ← one event per line, flushed immediately
│   └── meta.json        ← written atomically after session completes
├── 1743300100/
│   ├── events.jsonl
│   └── meta.json
└── 1743299900/
    └── events.jsonl     ← partial session (crashed or in-progress)
```

## Components

### `SessionStore` — Batch Read/Write

Used by `cmd_sessions` and by ingesters after normalizing external data.

```
SessionStore::open(root)
  │
  ├─ .write(&id, &[Event])
  │    events.jsonl  ← serialize each event as JSONL
  │    meta.json.tmp ← write meta, then rename (atomic)
  │
  ├─ .load(&id) → Vec<Event>
  │    reads events.jsonl line-by-line
  │    silently skips lines that fail to deserialize (forward compat)
  │
  ├─ .load_meta(&id) → Option<SessionMeta>
  │    reads meta.json if present
  │
  ├─ .list() → Vec<SessionId>
  │    reads all session dirs
  │    sorts newest-first by written_at
  │
  └─ .prune(keep) → usize
       deletes oldest sessions
       returns count deleted
```

### `SessionWriter` — Streaming Write

Used by `cmd_run` via `Engine::with_event_callback` to persist events incrementally during a live session.

```
SessionWriter::open(root, &id)
  │  creates {root}/{id}/ directory
  │  opens events.jsonl in append mode
  │
  ├─ .write_event(&Event)          ← called once per event, flushes immediately
  │    serialize as JSONL line
  │    writeln! + flush
  │
  └─ .finish()                     ← called after engine.run() returns
       compute event_count
       write meta.json.tmp
       rename → meta.json  (atomic)
       consume self
```

**Crash safety:** Events are on disk the moment `write_event` returns. If the process dies before `finish()`, the session directory exists with `events.jsonl` but no `meta.json`. `SessionStore::load()` can still read it; `list()` and `prune()` use `written_at` from `meta.json` so incomplete sessions are excluded from ordering.

## Write Path (cmd_run)

```
Engine emits event
    │
    ▼  event_callback
event_pipeline.redact_event(&event)    ← secrets stripped
    │
    ▼
SessionWriter.write_event(&redacted)   ← serialized + flushed to disk
    │
    ▼  [after engine.run() returns]
SessionWriter.finish()                 ← meta.json written atomically
```

## Read Path (cmd_sessions show)

```
SessionStore.load(&id)          → Vec<Event>
SessionStore.load_meta(&id)     → Option<SessionMeta>
    │
    ▼
render_session(&events, meta.as_ref(), &mut stdout)
    │
    ▼
Session: 1743300000  (2026-03-30 03:00:00 UTC)  5 events
--------------------------------------------------
   1  SessionStarted
   2  ProviderResponded
   3  ToolCalled          bash
   4  ToolCompleted       bash
   5  SessionCompleted
```

## Ingestion Pipeline

For importing sessions from external tools (Claude Code, devloop, etc.).

```
External JSONL file
    │
    ▼  Ingester::ingest(&source, &store)
    │
    ├─ BraidIngester       ← braid-native JSONL (pass-through)
    ├─ ClaudeCodeIngester  ← Claude Code conversation logs
    └─ DevloopIngester     ← devloop run transcripts
    │
    ▼  normalized Vec<Event>
    │
    ▼  SessionStore::write(&id, &events)
```

### Ingester Format Mappings

**Claude Code** (`.claude/projects/*/conversations/*.jsonl`):

| Source field | Mapped to |
|---|---|
| `type: "summary"` | `SessionStarted` |
| `type: "assistant"` | `ProviderResponded` |
| `type: "tool_use"` | `ToolCalled { tool_name }` |
| `type: "tool_result"` | `ToolCompleted { tool_name }` |
| (implicit at end) | `SessionCompleted` |
| `session_id` field | `SessionId` |

**Devloop** (run transcript JSONL):

| Source field | Mapped to |
|---|---|
| `event: "run_started"` | `SessionStarted` |
| `event: "llm_response"` | `ProviderResponded` |
| `event: "tool_call"` + `tool` | `ToolCalled { tool_name }` |
| `event: "tool_result"` + `tool` | `ToolCompleted { tool_name }` |
| `event: "run_completed"` | `SessionCompleted` |
| `run_id` field | `SessionId` (prefixed `devloop-`) |

## ReplaySession — Inspection Layer

`ReplaySession` wraps a loaded session for TUI and tooling consumption. It reads JSONL directly (not via `SessionStore::load`) to preserve the raw JSON payload of each event.

```
ReplaySession::load(&store, &id)
    │  reads events.jsonl line-by-line
    │  assigns 1-based index to each parseable line
    │  stores both the deserialized Event AND the raw serde_json::Value
    │
    ▼  Vec<ReplayEvent { index, event, payload }>

replay.get(1)     → Some(&ReplayEvent)   (1-based; get(0) → None)
replay.iter()     → Iterator<&ReplayEvent>
replay.len()      → usize
replay.is_empty() → bool
```

The `payload` field is the full raw JSON object for each event line. This allows TUI tools to inspect the exact on-disk representation, including any extra fields added by future versions of braid.

## Forward Compatibility

Unknown variants in `events.jsonl` are silently skipped by `SessionStore::load()`. This means a session written by a newer version of braid can be read by an older version — known events are preserved, future events are ignored.

The `Unknown { raw: String }` variant is not auto-produced by deserialization. It exists for explicit use by migration tooling that wants to round-trip unrecognized events through the store without losing them.
