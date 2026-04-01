# braid-observe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `braid-observe` crate — a session event store with a plain-ASCII inspector — and wire it into `braid-cli`.

**Architecture:** `SessionStore` persists events as JSONL + JSON meta per session directory; `render_session()` formats a human-readable timeline to any `Write` target. The crate depends only on `braid-model`; `braid-cli` handles redaction before calling `store.write()`.

**Tech Stack:** Rust 2024, `serde`/`serde_json` (workspace), `anyhow`/`thiserror` (workspace), `tempfile` (dev-only), `clap` (CLI side, already in workspace).

---

## File Map

### New files (braid-observe)

| File | Responsibility |
|---|---|
| `crates/braid-observe/Cargo.toml` | Crate manifest; workspace deps + `tempfile` dev-dep |
| `crates/braid-observe/src/lib.rs` | `pub use` re-exports |
| `crates/braid-observe/src/store.rs` | `SessionStore`, `SessionMeta` — all disk I/O |
| `crates/braid-observe/src/render.rs` | `render_session()` — plain-ASCII formatting |

### Modified files

| File | Change |
|---|---|
| `Cargo.toml` | Add `braid-observe` to workspace members |
| `crates/braid-cli/Cargo.toml` | Add `braid-observe` dependency |
| `crates/braid-cli/src/main.rs` | Wire `SessionStore` + `render_session` into `cmd_run`; add `Sessions` subcommand |

---

## Task 1: Scaffold the crate

**Files:**
- Create: `crates/braid-observe/Cargo.toml`
- Create: `crates/braid-observe/src/lib.rs`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Create `crates/braid-observe/Cargo.toml`**

```toml
[package]
name = "braid-observe"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
braid-model = { path = "../braid-model" }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create `crates/braid-observe/src/lib.rs`** (empty re-export shell)

```rust
pub mod render;
pub mod store;

pub use render::render_session;
pub use store::{SessionMeta, SessionStore};
```

- [ ] **Step 3: Add workspace member in root `Cargo.toml`**

Add `"crates/braid-observe"` to the `members` array in `Cargo.toml`.

- [ ] **Step 4: Verify it compiles (lib.rs will fail — that's fine for now)**

```bash
cargo check -p braid-observe
```

Expected: errors about missing modules `render` and `store` — that's expected. We're confirming the workspace registration works.

---

## Task 2: `SessionStore` — write and load

**Files:**
- Create: `crates/braid-observe/src/store.rs`

All tests in this task live inside `store.rs` in a `#[cfg(test)]` module.

- [ ] **Step 1: Write the failing roundtrip test**

Create `crates/braid-observe/src/store.rs` with just the test:

```rust
use anyhow::Result;
use braid_model::{Event, SessionId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: SessionId,
    pub written_at: String, // RFC 3339
    pub event_count: usize,
}

pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        todo!()
    }

    pub fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        todo!()
    }

    pub fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        todo!()
    }

    pub fn load_meta(&self, id: &SessionId) -> Result<Option<SessionMeta>> {
        todo!()
    }

    pub fn list(&self) -> Result<Vec<SessionId>> {
        todo!()
    }

    pub fn prune(&self, keep: usize) -> Result<usize> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::EventKind;

    fn make_events(session_id: &str) -> Vec<Event> {
        vec![
            Event { session_id: SessionId(session_id.into()), kind: EventKind::SessionStarted },
            Event { session_id: SessionId(session_id.into()), kind: EventKind::ProviderResponded },
            Event { session_id: SessionId(session_id.into()), kind: EventKind::SessionCompleted },
        ]
    }

    #[test]
    fn writes_and_loads_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let id = SessionId("sess-1".into());
        let events = make_events("sess-1");
        store.write(&id, &events).unwrap();
        let loaded = store.load(&id).unwrap();
        assert_eq!(loaded, events);
    }
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test -p braid-observe store::tests::writes_and_loads_roundtrip 2>&1 | tail -5
```

Expected: `not yet implemented` panic from `todo!()`.

- [ ] **Step 3: Implement `open`, `write`, `load`**

Replace the `todo!()`s with real implementations:

```rust
use anyhow::Result;
use braid_model::{Event, SessionId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: SessionId,
    pub written_at: String, // RFC 3339
    pub event_count: usize,
}

pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        let dir = self.session_dir(id);
        fs::create_dir_all(&dir)?;

        // Write events.jsonl
        let events_path = dir.join("events.jsonl");
        let mut f = fs::File::create(&events_path)?;
        for event in events {
            let line = serde_json::to_string(event)?;
            writeln!(f, "{}", line)?;
        }

        // Write meta.json atomically (write-then-rename)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        // Format as RFC 3339 without external deps: YYYY-MM-DDTHH:MM:SSZ
        let secs = now.as_secs();
        let written_at = format_rfc3339(secs);

        let meta = SessionMeta {
            session_id: id.clone(),
            written_at,
            event_count: events.len(),
        };
        let meta_json = serde_json::to_string(&meta)?;
        let tmp_path = dir.join("meta.json.tmp");
        fs::write(&tmp_path, &meta_json)?;
        fs::rename(&tmp_path, dir.join("meta.json"))?;

        Ok(())
    }

    pub fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        let path = self.session_dir(id).join("events.jsonl");
        if !path.exists() {
            return Err(anyhow::anyhow!("session not found: {}", id.0));
        }
        let file = fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line)?;
            events.push(event);
        }
        Ok(events)
    }

    pub fn load_meta(&self, id: &SessionId) -> Result<Option<SessionMeta>> {
        let path = self.session_dir(id).join("meta.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let meta: SessionMeta = serde_json::from_str(&content)?;
        Ok(Some(meta))
    }

    pub fn list(&self) -> Result<Vec<SessionId>> {
        let mut entries: Vec<(SessionId, String)> = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = SessionId(entry.file_name().to_string_lossy().into_owned());
            let written_at = self
                .load_meta(&id)?
                .map(|m| m.written_at)
                .unwrap_or_else(|| {
                    // Fallback: directory mtime as RFC 3339
                    entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| format_rfc3339(d.as_secs()))
                        .unwrap_or_default()
                });
            entries.push((id, written_at));
        }
        // Sort newest first (RFC 3339 strings sort lexicographically)
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(entries.into_iter().map(|(id, _)| id).collect())
    }

    pub fn prune(&self, keep: usize) -> Result<usize> {
        let all = self.list()?;
        if all.len() <= keep {
            return Ok(0);
        }
        let to_delete = &all[keep..];
        let count = to_delete.len();
        for id in to_delete {
            let dir = self.session_dir(id);
            fs::remove_dir_all(&dir)?;
        }
        Ok(count)
    }

    fn session_dir(&self, id: &SessionId) -> PathBuf {
        self.root.join(&id.0)
    }
}

/// Format unix seconds as RFC 3339 (UTC, no sub-second, no external deps).
fn format_rfc3339(secs: u64) -> String {
    // Days since Unix epoch math
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Gregorian calendar calculation from days since 1970-01-01
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}
```

- [ ] **Step 4: Run roundtrip test — expect pass**

```bash
cargo test -p braid-observe store::tests::writes_and_loads_roundtrip
```

Expected: `test store::tests::writes_and_loads_roundtrip ... ok`

---

## Task 3: `SessionStore` — list, prune, and edge cases

**Files:**
- Modify: `crates/braid-observe/src/store.rs` (add tests to `#[cfg(test)]`)

- [ ] **Step 1: Add remaining store tests**

Append inside the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn list_returns_sessions_by_recency() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        // Write sessions with a small delay to get different written_at values
        for id_str in &["sess-a", "sess-b", "sess-c"] {
            let id = SessionId(id_str.to_string());
            store.write(&id, &make_events(id_str)).unwrap();
            // Ensure different written_at by sleeping 1 second between writes
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
        // Newest first: sess-c was written last
        assert_eq!(list[0].0, "sess-c");
        assert_eq!(list[2].0, "sess-a");
    }

    #[test]
    fn prune_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        for i in 0..5 {
            let id = SessionId(format!("sess-{i}"));
            store.write(&id, &make_events(&format!("sess-{i}"))).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let deleted = store.prune(3).unwrap();
        assert_eq!(deleted, 2);

        let remaining = store.list().unwrap();
        assert_eq!(remaining.len(), 3);
        // Newest 3 remain: sess-4, sess-3, sess-2
        assert_eq!(remaining[0].0, "sess-4");
    }

    #[test]
    fn load_missing_session_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let err = store.load(&SessionId("ghost".into())).unwrap_err();
        assert!(err.to_string().contains("ghost"));
    }

    #[test]
    fn load_meta_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        // Write events manually without meta
        let sess_dir = dir.path().join("orphan");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let id = SessionId("orphan".into());
        let events = make_events("orphan");
        let mut f = std::fs::File::create(sess_dir.join("events.jsonl")).unwrap();
        for e in &events {
            writeln!(f, "{}", serde_json::to_string(e).unwrap()).unwrap();
        }
        // load() should succeed (best-effort)
        let loaded = store.load(&id).unwrap();
        assert_eq!(loaded, events);
        // load_meta() returns None
        let meta = store.load_meta(&id).unwrap();
        assert!(meta.is_none());
    }

    #[test]
    fn prune_keep_all_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        for i in 0..3 {
            let id = SessionId(format!("s{i}"));
            store.write(&id, &make_events(&format!("s{i}"))).unwrap();
        }
        let deleted = store.prune(10).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(store.list().unwrap().len(), 3);
    }
```

Also add `use std::io::Write;` at the top of the test module if not already present:

```rust
    use std::io::Write;
```

- [ ] **Step 2: Run all store tests**

```bash
cargo test -p braid-observe store::tests
```

Expected: all 6 tests pass. The `list_returns_sessions_by_recency` and `prune_removes_oldest` tests sleep between writes to get distinct `written_at` timestamps — they will be slow (~5s each) but deterministic.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-observe/ Cargo.toml
git commit -m "feat(braid-observe): add SessionStore with write/load/list/prune"
```

---

## Task 4: `render_session()`

**Files:**
- Create: `crates/braid-observe/src/render.rs`

- [ ] **Step 1: Write failing render tests**

Create `crates/braid-observe/src/render.rs`:

```rust
use std::io::Write;

use anyhow::Result;
use braid_model::{Event, EventKind, SessionId};

use crate::store::SessionMeta;

pub fn render_session(
    events: &[Event],
    meta: Option<&SessionMeta>,
    out: &mut impl Write,
) -> Result<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::EventKind;

    fn all_event_kinds(session_id: &str) -> Vec<Event> {
        vec![
            Event { session_id: SessionId(session_id.into()), kind: EventKind::SessionStarted },
            Event { session_id: SessionId(session_id.into()), kind: EventKind::ProviderResponded },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::ToolCalled { tool_name: "echo".into() },
            },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::ToolCompleted { tool_name: "echo".into() },
            },
            Event { session_id: SessionId(session_id.into()), kind: EventKind::SessionCompleted },
        ]
    }

    #[test]
    fn renders_all_event_kinds() {
        let events = all_event_kinds("abc");
        let meta = SessionMeta {
            session_id: SessionId("abc".into()),
            written_at: "2026-03-24T05:00:00Z".into(),
            event_count: 5,
        };
        let mut out = Vec::new();
        render_session(&events, Some(&meta), &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();

        assert!(s.contains("Session: abc"), "should contain session ID");
        assert!(s.contains("2026-03-24"), "should contain date");
        assert!(s.contains("5 events"), "should contain event count");
        assert!(s.contains("SessionStarted"), "should list SessionStarted");
        assert!(s.contains("ProviderResponded"), "should list ProviderResponded");
        assert!(s.contains("ToolCalled"), "should list ToolCalled");
        assert!(s.contains("echo"), "should show tool name");
        assert!(s.contains("ToolCompleted"), "should list ToolCompleted");
        assert!(s.contains("SessionCompleted"), "should list SessionCompleted");
        // Index numbers
        assert!(s.contains("  1 "), "should have index 1");
        assert!(s.contains("  5 "), "should have index 5");
    }

    #[test]
    fn renders_gracefully_without_meta() {
        let events = all_event_kinds("xyz");
        let mut out = Vec::new();
        render_session(&events, None, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        // Should not crash; should not emit a timestamp line
        assert!(s.contains("SessionStarted"));
        assert!(!s.contains("2026-"), "should not have timestamp when meta absent");
    }

    #[test]
    fn separator_is_ascii_only() {
        let events = all_event_kinds("s");
        let meta = SessionMeta {
            session_id: SessionId("s".into()),
            written_at: "2026-01-01T00:00:00Z".into(),
            event_count: events.len(),
        };
        let mut out = Vec::new();
        render_session(&events, Some(&meta), &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        // Separator line should only contain ASCII '-' characters
        let sep_line = s.lines().find(|l| l.contains("---")).unwrap();
        assert!(sep_line.is_ascii(), "separator must be ASCII only");
    }
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test -p braid-observe render::tests 2>&1 | tail -5
```

Expected: `not yet implemented`

- [ ] **Step 3: Implement `render_session`**

Replace `todo!()`:

```rust
pub fn render_session(
    events: &[Event],
    meta: Option<&SessionMeta>,
    out: &mut impl Write,
) -> Result<()> {
    // Header line
    let event_count = events.len();
    match meta {
        Some(m) => {
            // Parse written_at: "2026-03-24T05:00:00Z" → "2026-03-24 05:00:00 UTC"
            let ts = m.written_at.replace('T', " ").trim_end_matches('Z').to_string() + " UTC";
            writeln!(out, "Session: {}  ({})  {} events", m.session_id.0, ts, event_count)?;
        }
        None => {
            // Derive session_id from first event, or use placeholder
            let sid = events.first().map(|e| e.session_id.0.as_str()).unwrap_or("unknown");
            writeln!(out, "Session: {}  {} events", sid, event_count)?;
        }
    }

    // Separator (ASCII, capped at 72 chars)
    let sep_len = 72.min(50);
    writeln!(out, "{}", "-".repeat(sep_len))?;

    // Event rows: fixed-width index (3 chars), kind (20 chars), optional detail
    for (i, event) in events.iter().enumerate() {
        let (kind_str, detail) = match &event.kind {
            EventKind::SessionStarted => ("SessionStarted", None),
            EventKind::ProviderResponded => ("ProviderResponded", None),
            EventKind::ToolCalled { tool_name } => ("ToolCalled", Some(tool_name.as_str())),
            EventKind::ToolCompleted { tool_name } => ("ToolCompleted", Some(tool_name.as_str())),
            EventKind::SessionCompleted => ("SessionCompleted", None),
        };
        match detail {
            Some(d) => writeln!(out, "  {:>2}  {:<20}{}", i + 1, kind_str, d)?,
            None => writeln!(out, "  {:>2}  {}", i + 1, kind_str)?,
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run render tests — expect pass**

```bash
cargo test -p braid-observe render::tests
```

Expected: all 3 tests pass.

- [ ] **Step 5: Run all braid-observe tests**

```bash
cargo test -p braid-observe
```

Expected: all tests pass (store + render).

- [ ] **Step 6: Commit**

```bash
git add crates/braid-observe/src/render.rs
git commit -m "feat(braid-observe): add render_session() plain-ASCII formatter"
```

---

## Task 5: Wire into `braid-cli` — persist after `cmd_run`

**Files:**
- Modify: `crates/braid-cli/Cargo.toml`
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Add `braid-observe` to CLI dependencies**

In `crates/braid-cli/Cargo.toml`, add under `[dependencies]`:

```toml
braid-observe = { path = "../braid-observe" }
```

- [ ] **Step 2: Add imports to `main.rs`**

At the top of `crates/braid-cli/src/main.rs`, add to the existing `use` block:

```rust
use braid_observe::SessionStore;
use braid_redact::{EnvVarRule, HomePathRule, RedactionPipeline, SecretPatternRule};
```

(`braid_redact` imports already exist; just ensure `braid_observe` is imported.)

- [ ] **Step 3: Add `default_store_dir()`**

Add this function anywhere before `cmd_run`:

```rust
fn default_store_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(std::path::PathBuf::from(home).join(".braid").join("sessions"))
}
```

- [ ] **Step 4: Generate a unique `SessionId` and persist events in `cmd_run`**

Replace the hardcoded `SessionId("session".into())` with a timestamp-based ID, and add post-run persistence. Update `cmd_run` to:

```rust
fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let pipeline = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    // Generate a unique session ID based on current time
    let session_id = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        SessionId(format!("{secs}"))
    };

    let engine = Engine::new(ToolRegistry::new(), provider)
        .with_redactor(move |msg| pipeline.redact_message(msg));

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

    // Persist events (non-fatal on failure)
    if let Ok(store_dir) = default_store_dir() {
        if let Ok(store) = SessionStore::open(store_dir) {
            let event_pipeline = RedactionPipeline::new()
                .with_rule(SecretPatternRule::new())
                .with_rule(EnvVarRule::new())
                .with_rule(HomePathRule::new());
            let redacted_events: Vec<_> = output.events.iter()
                .map(|e| event_pipeline.redact_event(e))
                .collect();
            if let Err(e) = store.write(&session_id, &redacted_events) {
                eprintln!("warn: could not persist session: {e}");
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

- [ ] **Step 5: Verify it compiles**

```bash
cargo check -p braid-cli
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-cli/Cargo.toml crates/braid-cli/src/main.rs
git commit -m "feat(braid-cli): persist session events via braid-observe after cmd_run"
```

---

## Task 6: `braid sessions` subcommands

**Files:**
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Add `Sessions` subcommand variants to the CLI**

In `main.rs`, add to the `Command` enum:

```rust
/// Manage stored sessions
Sessions {
    #[command(subcommand)]
    action: SessionsCommand,
},
```

And define `SessionsCommand`:

```rust
#[derive(Subcommand)]
enum SessionsCommand {
    /// List session IDs, newest first
    List,
    /// Print a session's event timeline
    Show {
        /// Session ID to display
        id: String,
    },
    /// Delete oldest sessions, keeping N most recent
    Prune {
        /// Number of sessions to keep
        #[arg(long, default_value = "50")]
        keep: usize,
    },
}
```

- [ ] **Step 2: Add `cmd_sessions` and wire into `main`**

Add the handler:

```rust
fn cmd_sessions(action: SessionsCommand) -> Result<()> {
    use braid_observe::render_session;

    let store_dir = default_store_dir()?;
    let store = SessionStore::open(store_dir)?;

    match action {
        SessionsCommand::List => {
            let ids = store.list()?;
            if ids.is_empty() {
                println!("no sessions found");
            } else {
                for id in ids {
                    println!("{}", id.0);
                }
            }
        }
        SessionsCommand::Show { id } => {
            let sid = braid_model::SessionId(id);
            let events = store.load(&sid)?;
            let meta = store.load_meta(&sid)?;
            render_session(&events, meta.as_ref(), &mut std::io::stdout())?;
        }
        SessionsCommand::Prune { keep } => {
            let deleted = store.prune(keep)?;
            println!("deleted {deleted} session(s)");
        }
    }
    Ok(())
}
```

Wire in `main()`:

```rust
Command::Sessions { action } => cmd_sessions(action),
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p braid-cli
```

Expected: no errors.

- [ ] **Step 4: Smoke-test the subcommand help**

```bash
cargo run -p braid-cli -- sessions --help
```

Expected: shows `list`, `show`, `prune` subcommands.

- [ ] **Step 5: Run full workspace tests**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-cli/src/main.rs
git commit -m "feat(braid-cli): add 'braid sessions list/show/prune' subcommands"
```

---

## Task 7: Final verification

- [ ] **Step 1: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings before proceeding.

- [ ] **Step 2: Run fmt check**

```bash
cargo fmt --all -- --check
```

If it fails, run `cargo fmt --all` and commit the formatting fix.

- [ ] **Step 3: Run full test suite**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Final commit (if fmt was needed)**

```bash
git add -p
git commit -m "style: cargo fmt after braid-observe implementation"
```
