use anyhow::Result;
use braid_observe::{ReplaySession, SessionStore};

use crate::keys::{AppState, KeyAction, handle_key};

/// Loaded session data — separate from AppState so it can be replaced on reload.
pub struct LoadedSession {
    pub replay: ReplaySession,
}

/// Load the selected session from the store. Returns None if store is empty.
pub fn load_session(store: &SessionStore, state: &AppState) -> Option<Result<LoadedSession>> {
    let id = state.sessions.get(state.selected_session)?;
    Some(ReplaySession::load(store, id).map(|replay| LoadedSession { replay }))
}

/// Apply key action and return (should_quit, session_changed).
pub fn apply_key(state: &mut AppState, action: KeyAction) -> (bool, bool) {
    let prev_session = state.selected_session;
    let should_quit = handle_key(state, action);
    let session_changed = state.selected_session != prev_session;
    (should_quit, session_changed)
}

/// Reload the session list from disk and clamp the selection.
fn refresh_sessions(store: &SessionStore, state: &mut AppState) -> Result<()> {
    state.sessions = store.list()?;
    if state.selected_session >= state.sessions.len() && !state.sessions.is_empty() {
        state.selected_session = state.sessions.len() - 1;
    }
    Ok(())
}

/// Reload the selected session, updating `loaded`, `timeline_len`, and `state.error`.
fn reload_session(store: &SessionStore, state: &mut AppState, loaded: &mut Option<LoadedSession>) {
    match load_session(store, state) {
        None => {
            *loaded = None;
            state.timeline_len = 0;
        }
        Some(Ok(l)) => {
            state.timeline_len = l.replay.len();
            state.error = None; // clear any previous error on success
            *loaded = Some(l);
        }
        Some(Err(e)) => {
            state.error = Some(format!("error: {e}"));
            *loaded = None;
            state.timeline_len = 0;
        }
    }
}

pub fn run(terminal: &mut ratatui::DefaultTerminal, store: SessionStore) -> Result<()> {
    use crossterm::event::{self, Event as CrossEvent, KeyCode, KeyEventKind};

    let mut state = AppState::new(store.list()?);

    let mut loaded: Option<LoadedSession> = None;
    reload_session(&store, &mut state, &mut loaded);

    loop {
        terminal.draw(|frame| crate::ui::render(frame, &state, loaded.as_ref()))?;

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

        if action == KeyAction::Reload {
            // Full refresh: re-read session list from disk, then reload selected session
            if let Err(e) = refresh_sessions(&store, &mut state) {
                state.error = Some(format!("error listing sessions: {e}"));
            }
            state.timeline_cursor = 0;
            state.detail = crate::keys::DetailState::Collapsed;
            reload_session(&store, &mut state, &mut loaded);
        } else if session_changed {
            state.timeline_cursor = 0;
            reload_session(&store, &mut state, &mut loaded);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{Event, EventKind, SessionId};

    fn make_store_with_sessions(n: usize) -> (tempfile::TempDir, SessionStore, Vec<SessionId>) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
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
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        // State points to a session ID that doesn't exist on disk
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
        let dir = tempfile::tempdir().unwrap();
        let _store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        // Start with 3 sessions
        let (_dir2, store2, ids) = make_store_with_sessions(3);
        let mut state = AppState::new(ids);
        state.selected_session = 2;

        // Simulate store with only 1 session (others pruned)
        let small_store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let id = SessionId("only".into());
        small_store
            .write(
                &id,
                &[Event {
                    session_id: id.clone(),
                    kind: EventKind::SessionStarted,
                }],
            )
            .unwrap();

        refresh_sessions(&small_store, &mut state).unwrap();
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(
            state.selected_session, 0,
            "selection clamped to last valid index"
        );

        let _ = store2; // keep alive
    }
}
