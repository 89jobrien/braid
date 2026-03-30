# braid-observe Phase 3a: Streaming Ingest, Normalization, Replay

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen `braid-observe` with a streaming `SessionWriter`, three ingest adapters (braid-native, Claude Code, devloop), and a `ReplaySession` layer — then wire streaming persistence into `cmd_run`.

**Architecture:** `SessionWriter` appends events to disk incrementally during a session (crash-safe). The `Ingester` trait is a domain port; three adapter structs implement it for different source formats. `ReplaySession` wraps a loaded session as an indexed, payload-preserving structure for TUI consumption. All new public API is re-exported from `braid-observe/src/lib.rs`. `Engine` gains an optional event callback so `cmd_run` can stream-write without coupling core to the store.

**Tech Stack:** Rust 2024, `serde`/`serde_json` (workspace), `anyhow`/`thiserror` (workspace), `tempfile` (dev-dep, already in `braid-observe`), `braid-model`, `braid-core`, `braid-observe`.

---

## File Map

### New files

| File | Responsibility |
|---|---|
| `crates/braid-observe/src/ingest.rs` | `Ingester` trait + `BraidIngester`, `ClaudeCodeIngester`, `DevloopIngester` adapters |
| `crates/braid-observe/src/replay.rs` | `ReplaySession`, `ReplayEvent` — indexed, payload-preserving session view |
| `crates/braid-observe/fixtures/braid-native.jsonl` | Fixture: braid-native JSONL for ingester tests |
| `crates/braid-observe/fixtures/claude-code.jsonl` | Fixture: Claude Code conversation JSONL for ingester tests |
| `crates/braid-observe/fixtures/devloop.jsonl` | Fixture: devloop run transcript JSONL for ingester tests |

### Modified files

| File | Change |
|---|---|
| `crates/braid-model/src/event.rs` | Add `EventKind::Unknown { raw: String }` variant |
| `crates/braid-observe/src/store.rs` | Add `SessionWriter` struct with `open`, `write_event`, `finish` |
| `crates/braid-observe/src/render.rs` | Handle `EventKind::Unknown` in `render_session` |
| `crates/braid-observe/src/lib.rs` | Re-export `ingest`, `replay`, `SessionWriter` |
| `crates/braid-core/src/engine.rs` | Add `Engine::with_event_callback` + call it in the run loop |
| `crates/braid-cli/src/main.rs` | Replace batch write with `SessionWriter` in `cmd_run` |

---

## Task 1: Add `EventKind::Unknown`

**Files:**
- Modify: `crates/braid-model/src/event.rs`
- Modify: `crates/braid-observe/src/render.rs`
- Modify: `crates/braid-observe/src/store.rs` (update golden test)

- [ ] **Step 1: Add the `Unknown` variant to `EventKind`**

In `crates/braid-model/src/event.rs`, add `Unknown` to the enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    SessionStarted,
    ToolCalled { tool_name: String },
    ToolCompleted { tool_name: String },
    ProviderResponded,
    SessionCompleted,
    Unknown { raw: String },
}
```

- [ ] **Step 2: Handle `Unknown` in `render_session`**

In `crates/braid-observe/src/render.rs`, add the `Unknown` arm to the match in `render_session`:

```rust
EventKind::Unknown { raw } => ("Unknown", Some(raw.as_str())),
```

The full match block becomes:

```rust
let (kind_str, detail) = match &event.kind {
    EventKind::SessionStarted => ("SessionStarted", None),
    EventKind::ProviderResponded => ("ProviderResponded", None),
    EventKind::ToolCalled { tool_name } => ("ToolCalled", Some(tool_name.as_str())),
    EventKind::ToolCompleted { tool_name } => ("ToolCompleted", Some(tool_name.as_str())),
    EventKind::SessionCompleted => ("SessionCompleted", None),
    EventKind::Unknown { raw } => ("Unknown", Some(raw.as_str())),
};
```

- [ ] **Step 3: Add a test for `Unknown` round-trip in `store.rs`**

Add this test inside the `#[cfg(test)] mod tests` block in `crates/braid-observe/src/store.rs`:

```rust
#[test]
fn unknown_event_kind_survives_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let sess_dir = dir.path().join("u1");
    std::fs::create_dir_all(&sess_dir).unwrap();
    let mut f = std::fs::File::create(sess_dir.join("events.jsonl")).unwrap();
    // Write an Unknown variant manually
    writeln!(
        f,
        r#"{{"session_id":"u1","kind":{{"Unknown":{{"raw":"{{\"FutureEvent\":{{\"x\":1}}}}"}}}}}}"#
    )
    .unwrap();

    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let events = store.load(&SessionId("u1".into())).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0].kind, EventKind::Unknown { .. }));
}
```

- [ ] **Step 4: Run all braid-model and braid-observe tests**

```bash
cargo nextest run -p braid-model -p braid-observe
```

Expected: all pass (the `forward_compat_skips_unknown_event_kind` test now skips lines that don't match any known variant, and `Unknown` is a known variant — so we need to update that test too).

- [ ] **Step 5: Update `forward_compat_skips_unknown_event_kind` test**

The existing test in `store.rs` writes a `ProviderStreaming` line that previously got skipped. Now that `Unknown` exists, unrecognized variants should still be skipped (they won't deserialize into `Unknown` automatically — serde won't do that). Verify the test still passes as-is. If it does, no change needed. If it fails, check that the `Unknown` variant only applies when explicitly written as `{"Unknown": {"raw": "..."}}`.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings (likely exhaustive match warnings in other match sites).

- [ ] **Step 7: Commit**

```bash
git add crates/braid-model/src/event.rs crates/braid-observe/src/render.rs crates/braid-observe/src/store.rs
git commit -m "feat(braid-model): add EventKind::Unknown for forward-compat ingestion"
```

---

## Task 2: `SessionWriter` — streaming append to JSONL

**Files:**
- Modify: `crates/braid-observe/src/store.rs`
- Modify: `crates/braid-observe/src/lib.rs`

- [ ] **Step 1: Write the failing `SessionWriter` test**

Add to the `#[cfg(test)] mod tests` block in `crates/braid-observe/src/store.rs`:

```rust
#[test]
fn session_writer_streams_events_incrementally() {
    let dir = tempfile::tempdir().unwrap();
    let id = SessionId("stream-1".into());

    let mut writer = SessionWriter::open(dir.path(), &id).unwrap();

    let e1 = Event {
        session_id: id.clone(),
        kind: EventKind::SessionStarted,
    };
    let e2 = Event {
        session_id: id.clone(),
        kind: EventKind::ProviderResponded,
    };

    writer.write_event(&e1).unwrap();
    // Events are on disk immediately — load mid-session
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let partial = store.load(&id).unwrap();
    assert_eq!(partial.len(), 1, "first event visible before finish");

    writer.write_event(&e2).unwrap();
    writer.finish().unwrap();

    let loaded = store.load(&id).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].kind, EventKind::SessionStarted);
    assert_eq!(loaded[1].kind, EventKind::ProviderResponded);
}

#[test]
fn session_writer_writes_meta_on_finish() {
    let dir = tempfile::tempdir().unwrap();
    let id = SessionId("stream-2".into());
    let mut writer = SessionWriter::open(dir.path(), &id).unwrap();
    writer
        .write_event(&Event {
            session_id: id.clone(),
            kind: EventKind::SessionStarted,
        })
        .unwrap();
    writer.finish().unwrap();

    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let meta = store.load_meta(&id).unwrap();
    assert!(meta.is_some(), "meta.json written after finish");
    assert_eq!(meta.unwrap().event_count, 1);
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cargo nextest run -p braid-observe store::tests::session_writer 2>&1 | tail -5
```

Expected: compile error — `SessionWriter` not defined yet.

- [ ] **Step 3: Implement `SessionWriter` in `store.rs`**

Add after the `SessionStore` impl block in `crates/braid-observe/src/store.rs`:

```rust
/// Streams events to disk one at a time during an active session.
/// Call `finish()` to write `meta.json`. Safe to drop without `finish()` —
/// partial events remain on disk for inspection, but `meta.json` is absent.
pub struct SessionWriter {
    session_id: SessionId,
    dir: PathBuf,
    file: fs::File,
    event_count: usize,
}

impl SessionWriter {
    pub fn open(root: &std::path::Path, id: &SessionId) -> Result<Self> {
        let dir = root.join(&id.0);
        fs::create_dir_all(&dir)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("events.jsonl"))?;
        Ok(Self {
            session_id: id.clone(),
            dir,
            file,
            event_count: 0,
        })
    }

    pub fn write_event(&mut self, event: &Event) -> Result<()> {
        let line = serde_json::to_string(event)?;
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        self.event_count += 1;
        Ok(())
    }

    pub fn finish(self) -> Result<()> {
        let written_at = format_rfc3339(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        let meta = SessionMeta {
            session_id: self.session_id,
            written_at,
            event_count: self.event_count,
        };
        let meta_json = serde_json::to_string(&meta)?;
        let tmp = self.dir.join("meta.json.tmp");
        fs::write(&tmp, &meta_json)?;
        fs::rename(&tmp, self.dir.join("meta.json"))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run the new tests**

```bash
cargo nextest run -p braid-observe store::tests::session_writer
```

Expected: both pass.

- [ ] **Step 5: Re-export `SessionWriter` from `lib.rs`**

In `crates/braid-observe/src/lib.rs`:

```rust
pub mod render;
pub mod store;

pub use render::render_session;
pub use store::{SessionMeta, SessionStore, SessionWriter};
```

- [ ] **Step 6: Run all braid-observe tests**

```bash
cargo nextest run -p braid-observe
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/braid-observe/src/store.rs crates/braid-observe/src/lib.rs
git commit -m "feat(braid-observe): add SessionWriter for streaming event persistence"
```

---

## Task 3: `Engine::with_event_callback`

**Files:**
- Modify: `crates/braid-core/src/engine.rs`

- [ ] **Step 1: Write failing test for event callback**

Add to the `#[cfg(test)] mod tests` block in `crates/braid-core/src/engine.rs`:

```rust
#[test]
fn event_callback_fires_for_each_event() {
    use std::sync::{Arc, Mutex};

    let fired: Arc<Mutex<Vec<EventKind>>> = Arc::new(Mutex::new(vec![]));
    let fired_clone = Arc::clone(&fired);

    let engine = Engine::new(
        crate::tools::StaticTool::new("echo", "out"),
        TestProvider,
    )
    .with_event_callback(move |e: &Event| {
        fired_clone.lock().unwrap().push(e.kind.clone());
    });

    engine
        .run(
            RunInput {
                session_id: SessionId("cb-1".into()),
                messages: vec![Message {
                    role: Role::User,
                    content: vec![ContentPart::Text { text: "hi".into() }],
                }],
                max_turns: None,
            },
            &SimpleLoopPlanner,
        )
        .unwrap();

    let kinds = fired.lock().unwrap();
    assert!(kinds.contains(&EventKind::SessionStarted));
    assert!(kinds.contains(&EventKind::SessionCompleted));
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cargo nextest run -p braid-core engine::tests::event_callback 2>&1 | tail -5
```

Expected: compile error — `with_event_callback` not defined.

- [ ] **Step 3: Add `with_event_callback` to `Engine`**

In `crates/braid-core/src/engine.rs`, add the callback type alias and field:

```rust
type Redactor = Box<dyn Fn(&Message) -> Message + Send + Sync + 'static>;
type EventCallback = Box<dyn Fn(&Event) + Send + Sync + 'static>;
```

Add `event_callback: Option<EventCallback>` to the `Engine` struct:

```rust
pub struct Engine<T, P> {
    tool_executor: T,
    provider: P,
    redactor: Option<Redactor>,
    event_callback: Option<EventCallback>,
}
```

Update `Engine::new` to initialize it:

```rust
pub fn new(tool_executor: T, provider: P) -> Self {
    Self {
        tool_executor,
        provider,
        redactor: None,
        event_callback: None,
    }
}
```

Add the builder method:

```rust
pub fn with_event_callback(
    mut self,
    f: impl Fn(&Event) + Send + Sync + 'static,
) -> Self {
    self.event_callback = Some(Box::new(f));
    self
}
```

- [ ] **Step 4: Call the callback in `run`**

Add a helper closure inside `run` that fires the callback (and pushes to `events`). Replace every `events.push(event)` call with a call to this helper. In the `run` method, add after `let mut events = Vec::new();`:

```rust
let mut emit = |event: Event| {
    if let Some(cb) = &self.event_callback {
        cb(&event);
    }
    events.push(event);
};
```

Then replace all `events.push(Event { ... })` calls with `emit(Event { ... })`. There are 5 push sites. After the replacement:

```rust
emit(Event {
    session_id: input.session_id.clone(),
    kind: EventKind::SessionStarted,
});

// ... inside CallProvider arm:
emit(Event {
    session_id: input.session_id.clone(),
    kind: EventKind::ProviderResponded,
});

// ... inside ExecuteTool arm (ToolCalled):
emit(Event {
    session_id: input.session_id.clone(),
    kind: EventKind::ToolCalled {
        tool_name: call.name.clone(),
    },
});

// ... inside ExecuteTool arm (ToolCompleted):
emit(Event {
    session_id: input.session_id.clone(),
    kind: EventKind::ToolCompleted {
        tool_name: call.name.clone(),
    },
});

// ... inside Finish arm:
emit(Event {
    session_id: input.session_id.clone(),
    kind: EventKind::SessionCompleted,
});
```

Note: `emit` captures `&self.event_callback` which requires `self` to be borrowed. Since `run` takes `&self`, use a local reference:

```rust
let callback = self.event_callback.as_ref();
let mut emit = |event: Event| {
    if let Some(cb) = callback {
        cb(&event);
    }
    events.push(event);
};
```

- [ ] **Step 5: Run the new test**

```bash
cargo nextest run -p braid-core engine::tests::event_callback_fires_for_each_event
```

Expected: passes.

- [ ] **Step 6: Run all braid-core tests**

```bash
cargo nextest run -p braid-core
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/braid-core/src/engine.rs
git commit -m "feat(braid-core): add Engine::with_event_callback for streaming event emission"
```

---

## Task 4: Wire `SessionWriter` into `cmd_run`

**Files:**
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Replace batch write with `SessionWriter` in `cmd_run`**

In `crates/braid-cli/src/main.rs`, update `cmd_run`. The key changes:

1. Create `SessionWriter` before calling `engine.run()`
2. Register `with_event_callback` that redacts and writes each event
3. Call `writer.finish()` after the run
4. Remove the old post-run batch write block

Replace the body of `cmd_run` with:

```rust
fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let msg_pipeline = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let session_id = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        SessionId(format!("{secs}"))
    };

    // Open a streaming writer before the engine runs
    let writer = match default_store_dir() {
        Ok(dir) => match SessionWriter::open(&dir, &session_id) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("warn: could not open session writer: {e}");
                None
            }
        },
        Err(_) => None,
    };

    // Wrap writer in a Mutex so the callback can mutate it
    let writer = std::sync::Mutex::new(writer);

    let event_pipeline = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let engine = Engine::new(ToolRegistry::new(), provider)
        .with_redactor(move |msg| msg_pipeline.redact_message(msg))
        .with_event_callback(move |event| {
            let redacted = event_pipeline.redact_event(event);
            if let Ok(mut guard) = writer.lock() {
                if let Some(w) = guard.as_mut() {
                    let _ = w.write_event(&redacted);
                }
            }
        });

    let output = engine.run(
        RunInput {
            session_id: session_id.clone(),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: prompt }],
            }],
            max_turns: None,
        },
        &SimpleLoopPlanner,
    )?;

    // Finalize meta.json — non-fatal if it fails
    // (writer was moved into the callback closure above; retrieve from RunOutput events path)
    // Note: writer is consumed by the closure. Finish is handled implicitly on drop,
    // but we want meta.json written. Restructure: use Arc<Mutex<Option<SessionWriter>>>.

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{response_text}");
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}
```

Wait — the writer is moved into the closure. To call `finish()` after the run, use `Arc<Mutex<Option<SessionWriter>>>`:

```rust
fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    use std::sync::{Arc, Mutex};

    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let msg_pipeline = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let session_id = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        SessionId(format!("{secs}"))
    };

    let writer: Arc<Mutex<Option<SessionWriter>>> = Arc::new(Mutex::new(
        default_store_dir()
            .ok()
            .and_then(|dir| SessionWriter::open(&dir, &session_id).ok()),
    ));
    let writer_cb = Arc::clone(&writer);

    let event_pipeline = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let engine = Engine::new(ToolRegistry::new(), provider)
        .with_redactor(move |msg| msg_pipeline.redact_message(msg))
        .with_event_callback(move |event| {
            let redacted = event_pipeline.redact_event(event);
            if let Ok(mut guard) = writer_cb.lock() {
                if let Some(w) = guard.as_mut() {
                    let _ = w.write_event(&redacted);
                }
            }
        });

    let output = engine.run(
        RunInput {
            session_id: session_id.clone(),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: prompt }],
            }],
            max_turns: None,
        },
        &SimpleLoopPlanner,
    )?;

    // Finalize meta.json
    if let Ok(mut guard) = writer.lock() {
        if let Some(w) = guard.take() {
            if let Err(e) = w.finish() {
                eprintln!("warn: could not finalize session: {e}");
            }
        }
    }

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{response_text}");
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}
```

Also add `SessionWriter` to the import at the top of `main.rs`:

```rust
use braid_observe::{SessionStore, SessionWriter};
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p braid-cli
```

Expected: no errors.

- [ ] **Step 3: Run all workspace tests**

```bash
cargo nextest run --workspace
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/braid-cli/src/main.rs
git commit -m "feat(braid-cli): stream events via SessionWriter during cmd_run"
```

---

## Task 5: `Ingester` trait and `BraidIngester`

**Files:**
- Create: `crates/braid-observe/src/ingest.rs`
- Create: `crates/braid-observe/fixtures/braid-native.jsonl`
- Modify: `crates/braid-observe/src/lib.rs`
- Modify: `crates/braid-observe/Cargo.toml` (ensure `tempfile` is dev-dep)

- [ ] **Step 1: Create the fixture file**

Create `crates/braid-observe/fixtures/braid-native.jsonl` with:

```jsonl
{"session_id":"fix-1","kind":"SessionStarted"}
{"session_id":"fix-1","kind":"ProviderResponded"}
{"session_id":"fix-1","kind":{"ToolCalled":{"tool_name":"echo"}}}
{"session_id":"fix-1","kind":{"ToolCompleted":{"tool_name":"echo"}}}
{"session_id":"fix-1","kind":"SessionCompleted"}
```

- [ ] **Step 2: Write the failing `BraidIngester` test**

Create `crates/braid-observe/src/ingest.rs`:

```rust
use std::path::Path;

use anyhow::Result;
use braid_model::{Event, EventKind, SessionId};

use crate::store::SessionStore;

/// Port: ingest events from an external source into the store.
pub trait Ingester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId>;
}

/// Adapter: ingest braid-native JSONL (already normalized).
pub struct BraidIngester;

impl Ingester for BraidIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        todo!()
    }
}

/// Adapter: ingest Claude Code conversation JSONL.
pub struct ClaudeCodeIngester;

impl Ingester for ClaudeCodeIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        todo!()
    }
}

/// Adapter: ingest devloop run transcript JSONL.
pub struct DevloopIngester;

impl Ingester for DevloopIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        // fixtures/ lives next to src/ in the crate root
        let manifest = env!("CARGO_MANIFEST_DIR");
        std::path::PathBuf::from(manifest).join("fixtures").join(name)
    }

    #[test]
    fn braid_ingester_loads_native_jsonl() {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path("braid-native.jsonl");

        let id = BraidIngester.ingest(&source, &store).unwrap();

        let events = store.load(&id).unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        assert_eq!(events[4].kind, EventKind::SessionCompleted);
    }
}
```

- [ ] **Step 3: Run to confirm failure**

```bash
cargo nextest run -p braid-observe ingest::tests::braid_ingester 2>&1 | tail -5
```

Expected: `not yet implemented` panic.

- [ ] **Step 4: Implement `BraidIngester`**

Replace the `BraidIngester` impl:

```rust
impl Ingester for BraidIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<Event>(&line) {
                events.push(event);
            }
            // Skip lines that don't parse — forward compat
        }

        if events.is_empty() {
            anyhow::bail!("no events found in {}", source.display());
        }

        // Derive session ID from first event
        let id = events[0].session_id.clone();
        store.write(&id, &events)?;
        Ok(id)
    }
}
```

- [ ] **Step 5: Run test — expect pass**

```bash
cargo nextest run -p braid-observe ingest::tests::braid_ingester_loads_native_jsonl
```

Expected: passes.

- [ ] **Step 6: Re-export from `lib.rs`**

In `crates/braid-observe/src/lib.rs`:

```rust
pub mod ingest;
pub mod render;
pub mod store;

pub use ingest::{BraidIngester, ClaudeCodeIngester, DevloopIngester, Ingester};
pub use render::render_session;
pub use store::{SessionMeta, SessionStore, SessionWriter};
```

- [ ] **Step 7: Commit**

```bash
git add crates/braid-observe/src/ingest.rs crates/braid-observe/fixtures/braid-native.jsonl crates/braid-observe/src/lib.rs
git commit -m "feat(braid-observe): add Ingester trait and BraidIngester adapter"
```

---

## Task 6: `ClaudeCodeIngester`

**Files:**
- Create: `crates/braid-observe/fixtures/claude-code.jsonl`
- Modify: `crates/braid-observe/src/ingest.rs`

Claude Code conversation JSONL format (from `.claude/projects/*/conversations/*.jsonl`):

```jsonl
{"type":"summary","summary":"session started","session_id":"cc-abc"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello"}]},"session_id":"cc-abc"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi there"}]},"session_id":"cc-abc"}
{"type":"tool_use","tool_name":"read_file","session_id":"cc-abc"}
{"type":"tool_result","tool_name":"read_file","session_id":"cc-abc"}
```

- [ ] **Step 1: Create the Claude Code fixture**

Create `crates/braid-observe/fixtures/claude-code.jsonl`:

```jsonl
{"type":"summary","summary":"session started","session_id":"cc-abc"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello"}]},"session_id":"cc-abc"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi there"}]},"session_id":"cc-abc"}
{"type":"tool_use","tool_name":"read_file","session_id":"cc-abc"}
{"type":"tool_result","tool_name":"read_file","session_id":"cc-abc"}
```

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests` in `ingest.rs`:

```rust
#[test]
fn claude_code_ingester_normalizes_conversation() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let source = fixture_path("claude-code.jsonl");

    let id = ClaudeCodeIngester.ingest(&source, &store).unwrap();

    let events = store.load(&id).unwrap();
    // summary → SessionStarted, user+assistant → ProviderResponded,
    // tool_use → ToolCalled, tool_result → ToolCompleted, implicit SessionCompleted
    assert!(events.len() >= 3, "expected at least 3 normalized events");
    assert_eq!(events[0].kind, EventKind::SessionStarted);
    let has_tool_called = events
        .iter()
        .any(|e| matches!(&e.kind, EventKind::ToolCalled { tool_name } if tool_name == "read_file"));
    assert!(has_tool_called, "expected ToolCalled for read_file");
}
```

- [ ] **Step 3: Run to confirm failure**

```bash
cargo nextest run -p braid-observe ingest::tests::claude_code_ingester 2>&1 | tail -5
```

Expected: `not yet implemented` panic.

- [ ] **Step 4: Implement `ClaudeCodeIngester`**

Replace the `ClaudeCodeIngester` impl:

```rust
impl Ingester for ClaudeCodeIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);

        // Derive session ID from first line with session_id field
        let mut session_id: Option<SessionId> = None;
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };

            // Extract session_id from the first record that has it
            if session_id.is_none() {
                if let Some(sid) = val.get("session_id").and_then(|v| v.as_str()) {
                    session_id = Some(SessionId(sid.to_owned()));
                }
            }

            let sid = match &session_id {
                Some(s) => s.clone(),
                None => continue,
            };

            let kind = match val.get("type").and_then(|t| t.as_str()) {
                Some("summary") => EventKind::SessionStarted,
                Some("assistant") => EventKind::ProviderResponded,
                Some("tool_use") => {
                    let tool_name = val
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCalled { tool_name }
                }
                Some("tool_result") => {
                    let tool_name = val
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCompleted { tool_name }
                }
                // "user" and unknown types are skipped
                _ => continue,
            };

            events.push(Event { session_id: sid, kind });
        }

        let id = session_id.ok_or_else(|| anyhow::anyhow!("no session_id found in {}", source.display()))?;

        // Append implicit SessionCompleted if not already present
        if !matches!(events.last().map(|e| &e.kind), Some(EventKind::SessionCompleted)) {
            events.push(Event {
                session_id: id.clone(),
                kind: EventKind::SessionCompleted,
            });
        }

        store.write(&id, &events)?;
        Ok(id)
    }
}
```

- [ ] **Step 5: Run test — expect pass**

```bash
cargo nextest run -p braid-observe ingest::tests::claude_code_ingester_normalizes_conversation
```

Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-observe/src/ingest.rs crates/braid-observe/fixtures/claude-code.jsonl
git commit -m "feat(braid-observe): implement ClaudeCodeIngester adapter"
```

---

## Task 7: `DevloopIngester`

**Files:**
- Create: `crates/braid-observe/fixtures/devloop.jsonl`
- Modify: `crates/braid-observe/src/ingest.rs`

Devloop transcript format (from devloop run logs):

```jsonl
{"event":"run_started","run_id":"dl-xyz","ts":"2026-03-30T03:00:00Z"}
{"event":"llm_request","run_id":"dl-xyz","ts":"2026-03-30T03:00:01Z"}
{"event":"llm_response","run_id":"dl-xyz","ts":"2026-03-30T03:00:02Z"}
{"event":"tool_call","tool":"bash","run_id":"dl-xyz","ts":"2026-03-30T03:00:03Z"}
{"event":"tool_result","tool":"bash","run_id":"dl-xyz","ts":"2026-03-30T03:00:04Z"}
{"event":"run_completed","run_id":"dl-xyz","ts":"2026-03-30T03:00:05Z"}
```

- [ ] **Step 1: Create the devloop fixture**

Create `crates/braid-observe/fixtures/devloop.jsonl`:

```jsonl
{"event":"run_started","run_id":"dl-xyz","ts":"2026-03-30T03:00:00Z"}
{"event":"llm_request","run_id":"dl-xyz","ts":"2026-03-30T03:00:01Z"}
{"event":"llm_response","run_id":"dl-xyz","ts":"2026-03-30T03:00:02Z"}
{"event":"tool_call","tool":"bash","run_id":"dl-xyz","ts":"2026-03-30T03:00:03Z"}
{"event":"tool_result","tool":"bash","run_id":"dl-xyz","ts":"2026-03-30T03:00:04Z"}
{"event":"run_completed","run_id":"dl-xyz","ts":"2026-03-30T03:00:05Z"}
```

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests` in `ingest.rs`:

```rust
#[test]
fn devloop_ingester_normalizes_run_transcript() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let source = fixture_path("devloop.jsonl");

    let id = DevloopIngester.ingest(&source, &store).unwrap();

    let events = store.load(&id).unwrap();
    assert_eq!(events[0].kind, EventKind::SessionStarted);
    let has_tool = events
        .iter()
        .any(|e| matches!(&e.kind, EventKind::ToolCalled { tool_name } if tool_name == "bash"));
    assert!(has_tool, "expected ToolCalled for bash");
    assert_eq!(events.last().unwrap().kind, EventKind::SessionCompleted);
}
```

- [ ] **Step 3: Run to confirm failure**

```bash
cargo nextest run -p braid-observe ingest::tests::devloop_ingester 2>&1 | tail -5
```

Expected: `not yet implemented` panic.

- [ ] **Step 4: Implement `DevloopIngester`**

Replace the `DevloopIngester` impl:

```rust
impl Ingester for DevloopIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);
        let mut session_id: Option<SessionId> = None;
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };

            if session_id.is_none() {
                if let Some(rid) = val.get("run_id").and_then(|v| v.as_str()) {
                    session_id = Some(SessionId(format!("devloop-{}", rid)));
                }
            }

            let sid = match &session_id {
                Some(s) => s.clone(),
                None => continue,
            };

            let kind = match val.get("event").and_then(|t| t.as_str()) {
                Some("run_started") => EventKind::SessionStarted,
                Some("llm_response") => EventKind::ProviderResponded,
                Some("tool_call") => {
                    let tool_name = val
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCalled { tool_name }
                }
                Some("tool_result") => {
                    let tool_name = val
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCompleted { tool_name }
                }
                Some("run_completed") => EventKind::SessionCompleted,
                _ => continue,
            };

            events.push(Event { session_id: sid, kind });
        }

        let id = session_id.ok_or_else(|| anyhow::anyhow!("no run_id found in {}", source.display()))?;
        store.write(&id, &events)?;
        Ok(id)
    }
}
```

- [ ] **Step 5: Run test — expect pass**

```bash
cargo nextest run -p braid-observe ingest::tests::devloop_ingester_normalizes_run_transcript
```

Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-observe/src/ingest.rs crates/braid-observe/fixtures/devloop.jsonl
git commit -m "feat(braid-observe): implement DevloopIngester adapter"
```

---

## Task 8: `ReplaySession`

**Files:**
- Create: `crates/braid-observe/src/replay.rs`
- Modify: `crates/braid-observe/src/lib.rs`

- [ ] **Step 1: Write the failing `ReplaySession` tests**

Create `crates/braid-observe/src/replay.rs`:

```rust
use anyhow::Result;
use braid_model::{Event, SessionId};

use crate::store::SessionStore;

#[derive(Debug, Clone)]
pub struct ReplayEvent {
    pub index: usize, // 1-based, matching render output
    pub event: Event,
    pub payload: Option<serde_json::Value>,
}

pub struct ReplaySession {
    pub id: SessionId,
    events: Vec<ReplayEvent>,
}

impl ReplaySession {
    pub fn load(store: &SessionStore, id: &SessionId) -> Result<Self> {
        todo!()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReplayEvent> {
        self.events.iter()
    }

    pub fn get(&self, index: usize) -> Option<&ReplayEvent> {
        // index is 1-based
        self.events.get(index.saturating_sub(1))
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{EventKind, SessionId};
    use crate::store::SessionStore;

    fn make_store_with_session(session_id: &str) -> (tempfile::TempDir, SessionStore, SessionId) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let id = SessionId(session_id.into());
        let events = vec![
            braid_model::Event { session_id: id.clone(), kind: EventKind::SessionStarted },
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::ToolCalled { tool_name: "echo".into() },
            },
            braid_model::Event { session_id: id.clone(), kind: EventKind::SessionCompleted },
        ];
        store.write(&id, &events).unwrap();
        (dir, store, id)
    }

    #[test]
    fn load_returns_indexed_events() {
        let (_dir, store, id) = make_store_with_session("r1");
        let replay = ReplaySession::load(&store, &id).unwrap();
        assert_eq!(replay.len(), 3);
        assert_eq!(replay.get(1).unwrap().index, 1);
        assert_eq!(replay.get(1).unwrap().event.kind, EventKind::SessionStarted);
        assert_eq!(replay.get(3).unwrap().index, 3);
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let (_dir, store, id) = make_store_with_session("r2");
        let replay = ReplaySession::load(&store, &id).unwrap();
        assert!(replay.get(0).is_none(), "index 0 is out of range (1-based)");
        assert!(replay.get(99).is_none());
    }

    #[test]
    fn iter_yields_all_events_in_order() {
        let (_dir, store, id) = make_store_with_session("r3");
        let replay = ReplaySession::load(&store, &id).unwrap();
        let kinds: Vec<_> = replay.iter().map(|e| &e.event.kind).collect();
        assert_eq!(kinds[0], &EventKind::SessionStarted);
        assert_eq!(kinds[2], &EventKind::SessionCompleted);
    }

    #[test]
    fn payload_is_preserved_from_jsonl() {
        // Write a session directly as JSONL with extra fields
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let id = SessionId("r4".into());

        // Write manually so we control exact JSON
        let sess_dir = dir.path().join("r4");
        std::fs::create_dir_all(&sess_dir).unwrap();
        std::fs::write(
            sess_dir.join("events.jsonl"),
            r#"{"session_id":"r4","kind":"SessionStarted"}
{"session_id":"r4","kind":{"ToolCalled":{"tool_name":"echo"}}}
"#,
        )
        .unwrap();

        let replay = ReplaySession::load(&store, &id).unwrap();
        // payload for ToolCalled should be the raw JSON object
        let tool_event = replay.get(2).unwrap();
        let payload = tool_event.payload.as_ref().unwrap();
        assert_eq!(payload["kind"]["ToolCalled"]["tool_name"], "echo");
    }
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cargo nextest run -p braid-observe replay::tests 2>&1 | tail -5
```

Expected: compile error — `ReplaySession::load` is `todo!()`.

- [ ] **Step 3: Implement `ReplaySession::load`**

The key insight: `load` reads the raw JSONL lines to capture the full payload, not just the deserialized `Event`. Replace `todo!()`:

```rust
pub fn load(store: &SessionStore, id: &SessionId) -> Result<Self> {
    use std::io::BufRead;

    let root = store.root();
    let path = root.join(&id.0).join("events.jsonl");
    if !path.exists() {
        return Err(anyhow::anyhow!("session not found: {}", id.0));
    }

    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut events = Vec::new();
    let mut index = 0usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Event>(&line) else {
            continue;
        };
        let payload = serde_json::from_str::<serde_json::Value>(&line).ok();
        index += 1;
        events.push(ReplayEvent { index, event, payload });
    }

    Ok(Self { id: id.clone(), events })
}
```

This requires `SessionStore` to expose its `root` path. Add a method to `SessionStore` in `store.rs`:

```rust
impl SessionStore {
    // ... existing methods ...

    /// Expose the root directory for use by ReplaySession and other readers.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo nextest run -p braid-observe replay::tests
```

Expected: all pass.

- [ ] **Step 5: Re-export from `lib.rs`**

In `crates/braid-observe/src/lib.rs`:

```rust
pub mod ingest;
pub mod render;
pub mod replay;
pub mod store;

pub use ingest::{BraidIngester, ClaudeCodeIngester, DevloopIngester, Ingester};
pub use render::render_session;
pub use replay::{ReplayEvent, ReplaySession};
pub use store::{SessionMeta, SessionStore, SessionWriter};
```

- [ ] **Step 6: Run all workspace tests**

```bash
cargo nextest run --workspace
```

Expected: all pass.

- [ ] **Step 7: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/braid-observe/src/replay.rs crates/braid-observe/src/store.rs crates/braid-observe/src/lib.rs
git commit -m "feat(braid-observe): add ReplaySession with indexed, payload-preserving event view"
```

---

## Task 9: Final verification

- [ ] **Step 1: Run fmt check**

```bash
cargo fmt --all --check
```

If it fails, run `cargo fmt --all` and commit.

- [ ] **Step 2: Run full test suite**

```bash
cargo nextest run --workspace
```

Expected: all pass.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Smoke test the CLI**

```bash
op run --env-file=$HOME/.secrets -- cargo run -p braid-cli -- run "say hello"
cargo run -p braid-cli -- sessions list
```

Expected: a session appears in the list after the run.

- [ ] **Step 5: Final commit if any fmt changes**

```bash
git add -p
git commit -m "style: cargo fmt after phase 3a"
```
