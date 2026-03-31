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

pub fn run(terminal: &mut ratatui::DefaultTerminal, store: SessionStore) -> Result<()> {
    use crossterm::event::{self, Event as CrossEvent, KeyCode, KeyEventKind};

    let sessions = store.list()?;
    let mut state = AppState::new(sessions);

    let mut loaded = load_session(&store, &state).and_then(|r| r.ok());
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
            loaded = load_session(&store, &state).and_then(|r| match r {
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
}
