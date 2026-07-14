use std::sync::Arc;
use std::sync::mpsc;

use braid_observe::{ReplaySession, SessionStore};
use ratatui_bubbletea_components::TextInput;
use ratatui_tea::{Cmd, Model};

use crate::catalog::Catalog;
use crate::chat::{self, ChatMessage, EngineReply};
use crate::completion::CompletionState;
use crate::keys::{AppMode, AppState, DetailState, HarnessFocus, KeyAction, handle_key};

/// Loaded session data.
pub struct LoadedSession {
    pub replay: ReplaySession,
}

/// Load the selected session from the store. Returns None if store is empty.
pub fn load_session(
    store: &SessionStore,
    state: &AppState,
) -> Option<anyhow::Result<LoadedSession>> {
    let id = state.sessions.get(state.selected_session)?;
    Some(ReplaySession::load(store, id).map(|replay| LoadedSession { replay }))
}

/// Apply key action and return (`should_quit`, `session_changed`).
#[cfg(test)]
pub fn apply_key(state: &mut AppState, action: KeyAction) -> (bool, bool) {
    let prev_session = state.selected_session;
    let should_quit = handle_key(state, &action);
    let session_changed = state.selected_session != prev_session;
    (should_quit, session_changed)
}

/// Reload the session list from disk and clamp the selection.
pub fn refresh_sessions(store: &SessionStore, state: &mut AppState) -> anyhow::Result<()> {
    state.sessions = store.list()?;
    if state.selected_session >= state.sessions.len() && !state.sessions.is_empty() {
        state.selected_session = state.sessions.len() - 1;
    }
    Ok(())
}

/// Reload the selected session, updating `loaded`, `timeline_len`, and `state.error`.
pub fn reload_session(
    store: &SessionStore,
    state: &mut AppState,
    loaded: &mut Option<LoadedSession>,
) {
    match load_session(store, state) {
        None => {
            *loaded = None;
            state.timeline_len = 0;
        }
        Some(Ok(l)) => {
            state.timeline_len = l.replay.len();
            state.error = None;
            *loaded = Some(l);
        }
        Some(Err(e)) => {
            state.error = Some(format!("error: {e}"));
            *loaded = None;
            state.timeline_len = 0;
        }
    }
}

/// Top-level messages for the Elm update loop.
pub enum Msg {
    Key(crossterm::event::KeyEvent),
    EngineReply(EngineReply),
}

/// All application state — single source of truth.
pub struct AppModel {
    pub state: AppState,
    pub loaded: Option<LoadedSession>,
    pub chat_messages: Vec<ChatMessage>,
    pub input: TextInput,
    pub catalog: Catalog,
    pub completion: Option<CompletionState>,
    pub thinking: bool,
    pub arc_store: Arc<SessionStore>,
    pub model_name: String,
    pub rx: Option<mpsc::Receiver<EngineReply>>,
    pub should_quit: bool,
}

impl AppModel {
    pub fn new(store: Arc<SessionStore>, model_name: String) -> anyhow::Result<Self> {
        let sessions = store.list()?;
        let mut state = AppState::new(sessions);
        let mut loaded = None;
        reload_session(&store, &mut state, &mut loaded);

        let catalog = Catalog::load();

        let input = TextInput::new()
            .placeholder("type a message…  / for commands  @ for agents  $ for context");

        Ok(Self {
            state,
            loaded,
            chat_messages: Vec::new(),
            input,
            catalog,
            completion: None,
            thinking: false,
            arc_store: store,
            model_name,
            rx: None,
            should_quit: false,
        })
    }

    /// Poll the background engine thread, returning Some(msg) if a reply arrived.
    pub fn poll_engine(&mut self) -> Option<EngineReply> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok(reply) => {
                self.rx = None;
                Some(reply)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                None
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    fn handle_engine_reply(&mut self, reply: EngineReply) {
        match reply {
            EngineReply::Response(text) => {
                self.chat_messages.push(ChatMessage {
                    role: braid_model::Role::Assistant,
                    text,
                });
            }
            EngineReply::Error(e) => {
                self.chat_messages.push(ChatMessage {
                    role: braid_model::Role::Assistant,
                    text: format!("[error] {e}"),
                });
            }
        }
        self.thinking = false;
    }

    fn do_send(&mut self) {
        let text = self.input.state().value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // Clear input
        while !self.input.state().is_empty() {
            self.input.backspace();
        }
        self.completion = None;

        if let Some(rx) = chat::send(
            &text,
            Arc::clone(&self.arc_store),
            self.model_name.clone(),
            &mut self.chat_messages,
        ) {
            self.thinking = true;
            self.rx = Some(rx);
        }
    }

    fn do_type(&mut self, c: char) {
        self.input.insert(c);
        let input_val = self.input.state().value().to_string();
        chat::sync_completion(&input_val, &mut self.completion, &self.catalog);
    }

    fn do_backspace(&mut self) {
        self.input.backspace();
        let input_val = self.input.state().value().to_string();
        chat::sync_completion(&input_val, &mut self.completion, &self.catalog);
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

        if key.kind != KeyEventKind::Press {
            return false;
        }

        let in_chat = self.state.mode == AppMode::Harness
            && self.state.harness_focus == HarnessFocus::Chat
            && !self.thinking;
        let completion_open = self.completion.is_some();

        let action = if in_chat {
            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    KeyAction::Quit
                }
                KeyCode::Esc if completion_open => KeyAction::CompletionClose,
                KeyCode::Up if completion_open => KeyAction::CompletionUp,
                KeyCode::Down if completion_open => KeyAction::CompletionDown,
                KeyCode::Tab | KeyCode::Enter if completion_open => KeyAction::CompletionAccept,
                KeyCode::F(1) => KeyAction::SwitchMode,
                KeyCode::Tab => KeyAction::Tab,
                KeyCode::Enter => KeyAction::Send,
                KeyCode::Backspace => KeyAction::Backspace,
                KeyCode::Char(c) => KeyAction::Type(c),
                _ => KeyAction::Other,
            }
        } else {
            match key.code {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::F(1) => KeyAction::SwitchMode,
                KeyCode::Tab => KeyAction::Tab,
                KeyCode::Up => KeyAction::Up,
                KeyCode::Down => KeyAction::Down,
                KeyCode::Enter => KeyAction::Enter,
                KeyCode::Char('r') => KeyAction::Reload,
                _ => KeyAction::Other,
            }
        };

        // Handle harness-specific actions first
        match &action {
            KeyAction::Send => {
                self.do_send();
                return false;
            }
            KeyAction::Type(c) => {
                self.do_type(*c);
                return false;
            }
            KeyAction::Backspace => {
                self.do_backspace();
                return false;
            }
            KeyAction::CompletionUp => {
                if let Some(ref mut comp) = self.completion {
                    comp.move_up();
                }
                return false;
            }
            KeyAction::CompletionDown => {
                if let Some(ref mut comp) = self.completion {
                    comp.move_down();
                }
                return false;
            }
            KeyAction::CompletionAccept => {
                self.completion_accept();
                return false;
            }
            KeyAction::CompletionClose => {
                self.completion = None;
                return false;
            }
            _ => {}
        }

        // Catalog navigation when catalog is focused
        if self.state.mode == AppMode::Harness && self.state.harness_focus == HarnessFocus::Catalog
        {
            match action {
                KeyAction::Up => {
                    self.catalog.move_up();
                    return false;
                }
                KeyAction::Down => {
                    self.catalog.move_down();
                    return false;
                }
                _ => {}
            }
        }

        let prev_session = self.state.selected_session;
        let should_quit = handle_key(&mut self.state, &action);
        if should_quit {
            return true;
        }
        let session_changed = self.state.selected_session != prev_session;

        if action == KeyAction::Reload && self.state.mode == AppMode::Inspect {
            // Re-open store to refresh
            if let Ok(store) = SessionStore::open(self.arc_store.root().to_path_buf())
                && let Err(e) = refresh_sessions(&store, &mut self.state)
            {
                self.state.error = Some(format!("error listing sessions: {e}"));
            }
            self.state.timeline_cursor = 0;
            self.state.detail = DetailState::Collapsed;
            reload_session(&self.arc_store, &mut self.state, &mut self.loaded);
        } else if session_changed {
            self.state.timeline_cursor = 0;
            reload_session(&self.arc_store, &mut self.state, &mut self.loaded);
        }

        false
    }

    fn completion_accept(&mut self) {
        if let Some(ref comp) = self.completion
            && let Some(replacement) = comp.accept()
        {
            // Delete trigger char + filter length from the input
            let delete_count = 1 + comp.filter.chars().count();
            for _ in 0..delete_count {
                self.input.backspace();
            }
            for c in replacement.chars() {
                self.input.insert(c);
            }
        }
        self.completion = None;
    }
}

impl Model for AppModel {
    type Msg = Msg;

    fn init(&mut self) -> Cmd<Self::Msg> {
        Cmd::none()
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            Msg::Key(key) => {
                if self.handle_key_event(key) {
                    self.should_quit = true;
                }
            }
            Msg::EngineReply(reply) => {
                self.handle_engine_reply(reply);
            }
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut ratatui::Frame<'_>) {
        crate::ui::render(frame, &self.state, self.loaded.as_ref(), self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{Event, EventKind, SessionId};

    fn make_store_with_sessions(n: usize) -> (tempfile::TempDir, SessionStore, Vec<SessionId>) {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let mut ids = Vec::new();
        for i in 0..n {
            let id = SessionId(format!("s{i}"));
            let events = vec![
                Event {
                    session_id: id.clone(),
                    kind: EventKind::SessionStarted,
                },
                Event {
                    session_id: id.clone(),
                    kind: EventKind::SessionCompleted,
                },
            ];
            store.write(&id, &events).expect("should succeed");
            ids.push(id);
        }
        (dir, store, ids)
    }

    #[test]
    fn load_session_returns_none_for_empty_store() {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let state = AppState::new(vec![]);
        assert!(load_session(&store, &state).is_none());
    }

    #[test]
    fn load_session_loads_selected() {
        let (_dir, store, ids) = make_store_with_sessions(2);
        let mut state = AppState::new(ids);
        state.selected_session = 1;
        let loaded = load_session(&store, &state)
            .expect("should succeed")
            .expect("should succeed");
        assert_eq!(loaded.replay.id, SessionId("s1".into()));
        assert_eq!(loaded.replay.len(), 2);
    }

    #[test]
    fn apply_key_detects_session_change() {
        let (_dir, _store, ids) = make_store_with_sessions(2);
        let mut state = AppState::new(ids);
        state.mode = crate::keys::AppMode::Inspect;
        let (quit, changed) = apply_key(&mut state, KeyAction::Down);
        assert!(!quit);
        assert!(changed);
    }

    #[test]
    fn initial_state_has_first_session_selected() {
        let (_dir, store, ids) = make_store_with_sessions(3);
        let state = AppState::new(ids.clone());
        let loaded = load_session(&store, &state)
            .expect("should succeed")
            .expect("should succeed");
        assert_eq!(loaded.replay.id, ids[0]);
    }

    #[test]
    fn error_cleared_on_successful_reload() {
        let (_dir, store, ids) = make_store_with_sessions(1);
        let mut state = AppState::new(ids);
        state.error = Some("previous error".into());
        let mut loaded = None;
        reload_session(&store, &mut state, &mut loaded);
        assert!(state.error.is_none(), "error should be cleared on success");
        assert!(loaded.is_some());
    }

    #[test]
    fn error_set_when_session_missing() {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let mut state = AppState::new(vec![SessionId("ghost".into())]);
        let mut loaded = None;
        reload_session(&store, &mut state, &mut loaded);
        assert!(
            state.error.is_some(),
            "error should be set for missing session"
        );
        assert!(loaded.is_none());
    }

    #[test]
    fn refresh_sessions_updates_list_and_clamps_selection() {
        let dir = tempfile::tempdir().expect("should succeed");
        let _store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");

        // Start with 3 sessions
        let (_dir2, store2, ids) = make_store_with_sessions(3);
        let mut state = AppState::new(ids);
        state.selected_session = 2;

        // Simulate store with only 1 session
        let small_store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let id = SessionId("only".into());
        small_store
            .write(
                &id,
                &[Event {
                    session_id: id.clone(),
                    kind: EventKind::SessionStarted,
                }],
            )
            .expect("should succeed");

        refresh_sessions(&small_store, &mut state).expect("should succeed");
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(
            state.selected_session, 0,
            "selection clamped to last valid index"
        );

        let _ = store2; // keep alive
    }
}
