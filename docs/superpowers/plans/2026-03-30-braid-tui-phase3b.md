# braid-tui Phase 3b: Multi-Pane Session Inspector

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `braid-tui` — a standalone ratatui multi-pane inspector that shows a session list on the left, event timeline top-right, and an expandable detail pane bottom-right.

**Architecture:** Pure read-only binary crate. `AppState` is a plain struct holding all UI state — no ratatui types. `keys.rs` maps key events to state transitions (pure functions, easy to test). `ui.rs` renders `AppState` to a ratatui frame. `app.rs` drives the event loop. Reads exclusively from `braid-observe`'s public API (`SessionStore`, `ReplaySession`).

**Tech Stack:** Rust 2024, `ratatui` + `crossterm` (TUI), `braid-observe`, `braid-model`, `anyhow`. Depends on Phase 3a being complete (`ReplaySession` and `SessionStore::root()` must exist).

**Prerequisite:** Complete Phase 3a plan (`2026-03-30-braid-observe-phase3a.md`) before starting this plan.

---

## File Map

| File | Responsibility |
|---|---|
| `crates/braid-tui/Cargo.toml` | Crate manifest; ratatui, crossterm, braid-observe, braid-model, anyhow |
| `crates/braid-tui/src/main.rs` | Terminal setup/teardown, delegates to `app::run` |
| `crates/braid-tui/src/app.rs` | `AppState` struct, `run` event loop |
| `crates/braid-tui/src/keys.rs` | Key binding handler → pure state transitions |
| `crates/braid-tui/src/ui.rs` | ratatui render functions |
| `Cargo.toml` (root) | Add `braid-tui` to workspace members |

---

## Task 1: Scaffold the crate

**Files:**
- Create: `crates/braid-tui/Cargo.toml`
- Create: `crates/braid-tui/src/main.rs`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Create `crates/braid-tui/Cargo.toml`**

```toml
[package]
name = "braid-tui"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[[bin]]
name = "braid-tui"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-observe = { path = "../braid-observe" }
crossterm = "0.28"
ratatui = "0.29"
```

- [ ] **Step 2: Create `crates/braid-tui/src/main.rs`** (stub)

```rust
fn main() -> anyhow::Result<()> {
    println!("braid-tui: not yet implemented");
    Ok(())
}
```

- [ ] **Step 3: Add to workspace `Cargo.toml`**

In the root `Cargo.toml`, add `"crates/braid-tui"` to the `members` array.

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p braid-tui
```

Expected: compiles, prints stub message when run.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-tui/ Cargo.toml Cargo.lock
git commit -m "feat(braid-tui): scaffold crate with ratatui + crossterm deps"
```

---

## Task 2: `AppState` and key binding state machine

**Files:**
- Create: `crates/braid-tui/src/app.rs`
- Create: `crates/braid-tui/src/keys.rs`

- [ ] **Step 1: Write failing state machine tests**

Create `crates/braid-tui/src/keys.rs`:

```rust
use braid_model::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    SessionList,
    Timeline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailState {
    Collapsed,
    Expanded(usize), // 1-based ReplayEvent index
}

/// All UI state. No ratatui types — pure data, easy to test.
#[derive(Debug)]
pub struct AppState {
    pub sessions: Vec<SessionId>,
    pub selected_session: usize, // index into sessions (0-based)
    pub focus: Focus,
    pub timeline_len: usize,     // number of events in loaded session
    pub timeline_cursor: usize,  // 0-based index into timeline
    pub detail: DetailState,
    pub error: Option<String>,   // displayed in timeline pane on load failure
}

impl AppState {
    pub fn new(sessions: Vec<SessionId>) -> Self {
        Self {
            sessions,
            selected_session: 0,
            focus: Focus::SessionList,
            timeline_len: 0,
            timeline_cursor: 0,
            detail: DetailState::Collapsed,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Tab,
    Up,
    Down,
    Enter,
    Reload,
    Quit,
    Other,
}

/// Pure state transition — no I/O. Returns true if the app should quit.
pub fn handle_key(state: &mut AppState, action: KeyAction) -> bool {
    match action {
        KeyAction::Quit => return true,
        KeyAction::Tab => {
            state.focus = match state.focus {
                Focus::SessionList => Focus::Timeline,
                Focus::Timeline => Focus::SessionList,
            };
        }
        KeyAction::Up => match state.focus {
            Focus::SessionList => {
                if state.selected_session > 0 {
                    state.selected_session -= 1;
                    state.timeline_cursor = 0;
                    state.detail = DetailState::Collapsed;
                }
            }
            Focus::Timeline => {
                if state.timeline_cursor > 0 {
                    state.timeline_cursor -= 1;
                }
            }
        },
        KeyAction::Down => match state.focus {
            Focus::SessionList => {
                if state.selected_session + 1 < state.sessions.len() {
                    state.selected_session += 1;
                    state.timeline_cursor = 0;
                    state.detail = DetailState::Collapsed;
                }
            }
            Focus::Timeline => {
                if state.timeline_cursor + 1 < state.timeline_len {
                    state.timeline_cursor += 1;
                }
            }
        },
        KeyAction::Enter => {
            if state.focus == Focus::Timeline && state.timeline_len > 0 {
                let event_index = state.timeline_cursor + 1; // 1-based
                state.detail = match &state.detail {
                    DetailState::Collapsed => DetailState::Expanded(event_index),
                    DetailState::Expanded(i) if *i == event_index => DetailState::Collapsed,
                    DetailState::Expanded(_) => DetailState::Expanded(event_index),
                };
            }
        }
        KeyAction::Reload => {
            // Caller handles reload; state reset done externally
        }
        KeyAction::Other => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_sessions(n: usize) -> AppState {
        let sessions = (0..n).map(|i| SessionId(format!("s{i}"))).collect();
        AppState::new(sessions)
    }

    #[test]
    fn tab_cycles_focus() {
        let mut state = state_with_sessions(2);
        assert_eq!(state.focus, Focus::SessionList);
        handle_key(&mut state, KeyAction::Tab);
        assert_eq!(state.focus, Focus::Timeline);
        handle_key(&mut state, KeyAction::Tab);
        assert_eq!(state.focus, Focus::SessionList);
    }

    #[test]
    fn quit_returns_true() {
        let mut state = state_with_sessions(1);
        assert!(handle_key(&mut state, KeyAction::Quit));
    }

    #[test]
    fn down_in_session_list_advances_selection() {
        let mut state = state_with_sessions(3);
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.selected_session, 1);
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.selected_session, 2);
        // Clamps at end
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.selected_session, 2);
    }

    #[test]
    fn up_in_session_list_clamps_at_zero() {
        let mut state = state_with_sessions(2);
        handle_key(&mut state, KeyAction::Up);
        assert_eq!(state.selected_session, 0);
    }

    #[test]
    fn down_in_timeline_advances_cursor() {
        let mut state = state_with_sessions(1);
        state.focus = Focus::Timeline;
        state.timeline_len = 3;
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.timeline_cursor, 1);
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.timeline_cursor, 2);
        // Clamps
        handle_key(&mut state, KeyAction::Down);
        assert_eq!(state.timeline_cursor, 2);
    }

    #[test]
    fn enter_toggles_detail_on_same_row() {
        let mut state = state_with_sessions(1);
        state.focus = Focus::Timeline;
        state.timeline_len = 3;
        state.timeline_cursor = 1; // row 2 (0-based)

        handle_key(&mut state, KeyAction::Enter);
        assert_eq!(state.detail, DetailState::Expanded(2)); // 1-based

        handle_key(&mut state, KeyAction::Enter);
        assert_eq!(state.detail, DetailState::Collapsed);
    }

    #[test]
    fn enter_switches_detail_to_new_row() {
        let mut state = state_with_sessions(1);
        state.focus = Focus::Timeline;
        state.timeline_len = 3;
        state.timeline_cursor = 0;
        handle_key(&mut state, KeyAction::Enter);
        assert_eq!(state.detail, DetailState::Expanded(1));

        state.timeline_cursor = 2;
        handle_key(&mut state, KeyAction::Enter);
        assert_eq!(state.detail, DetailState::Expanded(3));
    }

    #[test]
    fn selecting_new_session_resets_cursor_and_detail() {
        let mut state = state_with_sessions(2);
        state.focus = Focus::Timeline;
        state.timeline_len = 5;
        state.timeline_cursor = 3;
        state.detail = DetailState::Expanded(4);

        state.focus = Focus::SessionList;
        handle_key(&mut state, KeyAction::Down);

        assert_eq!(state.timeline_cursor, 0);
        assert_eq!(state.detail, DetailState::Collapsed);
    }
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cargo nextest run -p braid-tui 2>&1 | tail -5
```

Expected: compile error — `keys.rs` not linked from `main.rs` yet.

- [ ] **Step 3: Update `main.rs` to declare modules**

```rust
mod app;
mod keys;
mod ui;

fn main() -> anyhow::Result<()> {
    println!("braid-tui: not yet implemented");
    Ok(())
}
```

Create stubs for `app.rs` and `ui.rs` so it compiles:

`crates/braid-tui/src/app.rs`:
```rust
// stub
```

`crates/braid-tui/src/ui.rs`:
```rust
// stub
```

- [ ] **Step 4: Run key binding tests**

```bash
cargo nextest run -p braid-tui keys::tests
```

Expected: all 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-tui/src/keys.rs crates/braid-tui/src/app.rs crates/braid-tui/src/ui.rs crates/braid-tui/src/main.rs
git commit -m "feat(braid-tui): add AppState and key binding state machine with tests"
```

---

## Task 3: `app.rs` — event loop and session loading

**Files:**
- Modify: `crates/braid-tui/src/app.rs`

- [ ] **Step 1: Write the app state integration test**

Add to `crates/braid-tui/src/app.rs`:

```rust
use anyhow::Result;
use braid_model::SessionId;
use braid_observe::{ReplaySession, SessionStore};

use crate::keys::{AppState, DetailState, Focus, KeyAction, handle_key};

/// Loaded session data — separate from AppState so it can be replaced on reload.
pub struct LoadedSession {
    pub replay: ReplaySession,
}

/// Load the selected session from the store. Returns None if store is empty.
pub fn load_session(store: &SessionStore, state: &AppState) -> Option<Result<LoadedSession>> {
    let id = state.sessions.get(state.selected_session)?;
    Some(ReplaySession::load(store, id).map(|replay| LoadedSession { replay }))
}

/// Apply key action and return whether the selected session changed (caller should reload).
pub fn apply_key(state: &mut AppState, action: KeyAction) -> (bool, bool) {
    let prev_session = state.selected_session;
    let should_quit = handle_key(state, action);
    let session_changed = state.selected_session != prev_session;
    (should_quit, session_changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{Event, EventKind};

    fn make_store_with_sessions(n: usize) -> (tempfile::TempDir, SessionStore, Vec<SessionId>) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let mut ids = Vec::new();
        for i in 0..n {
            let id = SessionId(format!("s{i}"));
            let events = vec![
                Event { session_id: id.clone(), kind: EventKind::SessionStarted },
                Event { session_id: id.clone(), kind: EventKind::SessionCompleted },
            ];
            store.write(&id, &events).unwrap();
            ids.push(id);
        }
        (dir, store, ids)
    }

    #[test]
    fn load_session_returns_none_for_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let state = AppState::new(vec![]);
        assert!(load_session(&store, &state).is_none());
    }

    #[test]
    fn load_session_loads_selected() {
        let (_dir, store, ids) = make_store_with_sessions(2);
        let mut state = AppState::new(ids);
        state.selected_session = 1;
        let loaded = load_session(&store, &state).unwrap().unwrap();
        assert_eq!(loaded.replay.id, SessionId("s1".into()));
        assert_eq!(loaded.replay.len(), 2);
    }

    #[test]
    fn apply_key_detects_session_change() {
        let (_dir, _store, ids) = make_store_with_sessions(2);
        let mut state = AppState::new(ids);
        let (quit, changed) = apply_key(&mut state, KeyAction::Down);
        assert!(!quit);
        assert!(changed);
    }

    #[test]
    fn initial_state_has_first_session_selected() {
        let (_dir, store, ids) = make_store_with_sessions(3);
        let state = AppState::new(ids.clone());
        let loaded = load_session(&store, &state).unwrap().unwrap();
        assert_eq!(loaded.replay.id, ids[0]);
    }
}
```

Also add `tempfile` to `braid-tui` dev-dependencies in `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p braid-tui app::tests
```

Expected: all 4 pass.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-tui/src/app.rs crates/braid-tui/Cargo.toml
git commit -m "feat(braid-tui): add app session loading and key dispatch with tests"
```

---

## Task 4: `ui.rs` — ratatui rendering

**Files:**
- Modify: `crates/braid-tui/src/ui.rs`

- [ ] **Step 1: Implement the render function**

Replace the stub in `crates/braid-tui/src/ui.rs`:

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::LoadedSession;
use crate::keys::{AppState, DetailState, Focus};


pub fn render(frame: &mut Frame, state: &AppState, loaded: Option<&LoadedSession>) {
    let size = frame.area();

    // Minimum size check
    if size.width < 80 || size.height < 24 {
        let msg = Paragraph::new("terminal too small (need 80×24)")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(msg, size);
        return;
    }

    // Top status bar (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(size);

    let status = Paragraph::new(Line::from(vec![
        Span::styled("braid inspect", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            "Tab: switch focus  ↑↓: navigate  Enter: expand  r: reload  q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(status, chunks[0]);

    // Main area: session list | right column
    let main = chunks[1];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(main);

    render_session_list(frame, state, cols[0]);
    render_right_column(frame, state, loaded, cols[1]);
}

fn render_session_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::SessionList;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = state
        .sessions
        .iter()
        .map(|id| ListItem::new(id.0.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Sessions"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !state.sessions.is_empty() {
        list_state.select(Some(state.selected_session));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_right_column(
    frame: &mut Frame,
    state: &AppState,
    loaded: Option<&LoadedSession>,
    area: Rect,
) {
    let show_detail = !matches!(state.detail, DetailState::Collapsed);

    let rows = if show_detail {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    render_timeline(frame, state, loaded, rows[0]);

    if show_detail {
        render_detail(frame, state, loaded, rows[1]);
    }
}

fn render_timeline(
    frame: &mut Frame,
    state: &AppState,
    loaded: Option<&LoadedSession>,
    area: Rect,
) {
    use braid_model::EventKind;

    let focused = state.focus == Focus::Timeline;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let title = match state.sessions.get(state.selected_session) {
        Some(id) => format!("Timeline  {}", id.0),
        None => "Timeline".into(),
    };

    if let Some(err) = &state.error {
        let p = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = match loaded {
        None => vec![ListItem::new("no sessions found")],
        Some(s) => s
            .replay
            .iter()
            .map(|re| {
                let (kind_str, detail) = match &re.event.kind {
                    EventKind::SessionStarted => ("SessionStarted", None),
                    EventKind::ProviderResponded => ("ProviderResponded", None),
                    EventKind::ToolCalled { tool_name } => ("ToolCalled", Some(tool_name.as_str())),
                    EventKind::ToolCompleted { tool_name } => ("ToolCompleted", Some(tool_name.as_str())),
                    EventKind::SessionCompleted => ("SessionCompleted", None),
                    EventKind::Unknown { raw } => ("Unknown", Some(raw.as_str())),
                };
                let expanded = matches!(&state.detail, DetailState::Expanded(i) if *i == re.index);
                let marker = if expanded { "▼" } else if detail.is_some() { "▶" } else { " " };
                let line = match detail {
                    Some(d) => format!("  {:>3}  {:<22}{} {}", re.index, kind_str, marker, d),
                    None => format!("  {:>3}  {}{}", re.index, kind_str, marker),
                };
                ListItem::new(line)
            })
            .collect(),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ListState::default();
    if loaded.is_some() && !state.sessions.is_empty() {
        list_state.select(Some(state.timeline_cursor));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_detail(
    frame: &mut Frame,
    state: &AppState,
    loaded: Option<&LoadedSession>,
    area: Rect,
) {
    let content = match (&state.detail, loaded) {
        (DetailState::Expanded(index), Some(s)) => {
            match s.replay.get(*index) {
                Some(re) => match &re.payload {
                    Some(val) => serde_json::to_string_pretty(val).unwrap_or_else(|_| "?".into()),
                    None => "(no payload)".into(),
                },
                None => "(event not found)".into(),
            }
        }
        _ => String::new(),
    };

    let p = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Detail"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(p, area);
}
```

Also add `serde_json` to `braid-tui/Cargo.toml` dependencies:

```toml
serde_json.workspace = true
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p braid-tui
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-tui/src/ui.rs crates/braid-tui/Cargo.toml
git commit -m "feat(braid-tui): implement ratatui render functions (session list, timeline, detail)"
```

---

## Task 5: Wire the event loop in `main.rs`

**Files:**
- Modify: `crates/braid-tui/src/main.rs`
- Modify: `crates/braid-tui/src/app.rs`

- [ ] **Step 1: Add the `run` function to `app.rs`**

Add to `crates/braid-tui/src/app.rs` (after existing code, before `#[cfg(test)]`):

```rust
use crossterm::event::{self, Event as CrossEvent, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

pub fn run(terminal: &mut DefaultTerminal, store: SessionStore) -> Result<()> {
    let sessions = store.list()?;
    let mut state = AppState::new(sessions);

    // Load the initially selected session
    let mut loaded = load_session(&store, &state)
        .and_then(|r| r.ok());

    // Sync timeline_len into state
    if let Some(ref l) = loaded {
        state.timeline_len = l.replay.len();
    }

    loop {
        let loaded_ref = loaded.as_ref();
        terminal.draw(|frame| crate::ui::render(frame, &state, loaded_ref))?;

        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        let CrossEvent::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        let action = match key.code {
            KeyCode::Char('q') => KeyAction::Quit,
            KeyCode::Tab => KeyAction::Tab,
            KeyCode::Up => KeyAction::Up,
            KeyCode::Down => KeyAction::Down,
            KeyCode::Enter => KeyAction::Enter,
            KeyCode::Char('r') => KeyAction::Reload,
            _ => KeyAction::Other,
        };

        let (should_quit, session_changed) = apply_key(&mut state, action.clone());
        if should_quit {
            break;
        }

        if session_changed || action == KeyAction::Reload {
            loaded = load_session(&store, &state)
                .and_then(|r| match r {
                    Ok(l) => Some(l),
                    Err(e) => {
                        state.error = Some(format!("error: {e}"));
                        None
                    }
                });
            state.timeline_len = loaded.as_ref().map(|l| l.replay.len()).unwrap_or(0);
            state.timeline_cursor = 0;
        }
    }

    Ok(())
}
```

Also add `use braid_observe::SessionStore;` and `use crate::keys::KeyAction;` to the imports at the top of `app.rs`.

- [ ] **Step 2: Wire `main.rs`**

Replace `crates/braid-tui/src/main.rs`:

```rust
mod app;
mod keys;
mod ui;

use anyhow::{Context, Result};
use braid_observe::SessionStore;

fn default_store_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(std::path::PathBuf::from(home).join(".braid").join("sessions"))
}

fn main() -> Result<()> {
    let store_dir = default_store_dir()?;
    let store = SessionStore::open(store_dir)?;

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, store);
    ratatui::restore();

    result
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo build -p braid-tui
```

Expected: compiles cleanly.

- [ ] **Step 4: Smoke test**

```bash
cargo run -p braid-tui
```

Expected: TUI opens showing session list. Navigate with arrow keys, Tab switches panes, Enter expands a row, `q` quits. If no sessions exist, shows "no sessions found" in the timeline pane. Verify terminal is restored cleanly on quit.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-tui/src/main.rs crates/braid-tui/src/app.rs
git commit -m "feat(braid-tui): wire event loop and terminal setup in main.rs"
```

---

## Task 6: Final verification

- [ ] **Step 1: Run all workspace tests**

```bash
cargo nextest run --workspace
```

Expected: all pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings (common: unused imports, dead code in TUI stubs).

- [ ] **Step 3: Run fmt**

```bash
cargo fmt --all --check
```

If it fails, run `cargo fmt --all` and commit.

- [ ] **Step 4: End-to-end smoke test**

```bash
# Run a session to populate the store
op run --env-file=$HOME/.secrets -- cargo run -p braid-cli -- run "say hello"

# Open the inspector
cargo run -p braid-tui
```

Expected:
- Session list shows the session just created
- Timeline shows SessionStarted, ProviderResponded, SessionCompleted
- Enter on ProviderResponded expands the detail pane with JSON payload
- `q` quits cleanly

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(braid-tui): complete multi-pane session inspector"
```
