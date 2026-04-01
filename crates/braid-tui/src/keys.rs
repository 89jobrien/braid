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
    pub timeline_len: usize,    // number of events in loaded session
    pub timeline_cursor: usize, // 0-based index into timeline
    pub detail: DetailState,
    pub error: Option<String>, // displayed in timeline pane on load failure
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
