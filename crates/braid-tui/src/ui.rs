use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::LoadedSession;
use crate::keys::{AppState, DetailState, Focus};

/// Strip ANSI/VT escape sequences and other non-printable characters from a string
/// before rendering, preventing escape injection from untrusted session payloads.
///
/// Sequences handled:
/// - `ESC [ <params> <final>` — CSI sequences (colors, cursor movement, etc.)
/// - `ESC <non-[>` — two-character Fe/Fs sequences
/// - bare ESC (incomplete or unrecognised) — dropped
/// - other control chars < 0x20 (except tab/newline) and DEL — replaced with `·`
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                // Consume the escape sequence without emitting it.
                match chars.peek() {
                    Some(&'[') => {
                        // CSI sequence: ESC [ <parameter bytes 0x30–0x3F>* <intermediate 0x20–0x2F>* <final 0x40–0x7E>
                        chars.next(); // consume '['
                        for ch in chars.by_ref() {
                            if ('\x40'..='\x7e').contains(&ch) {
                                break; // final byte consumed, sequence done
                            }
                            // parameter and intermediate bytes are skipped
                        }
                    }
                    Some(_) => {
                        // Two-character escape sequence — consume the second char.
                        chars.next();
                    }
                    None => {
                        // Bare ESC at end of string — drop it.
                    }
                }
            }
            '\t' | '\n' => out.push(c),
            c if (c as u32) < 0x20 || c == '\x7f' => out.push('·'),
            _ => out.push(c),
        }
    }

    out
}

pub fn render(frame: &mut Frame, state: &AppState, loaded: Option<&LoadedSession>) {
    let size = frame.area();

    if size.width < 80 || size.height < 24 {
        let msg = Paragraph::new("terminal too small (need 80×24)")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(msg, size);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(size);

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            "braid inspect",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "Tab: switch focus  ↑↓: navigate  Enter: expand  r: reload  q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(status, chunks[0]);

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
                    EventKind::ToolCompleted { tool_name } => {
                        ("ToolCompleted", Some(tool_name.as_str()))
                    }
                    EventKind::SessionCompleted => ("SessionCompleted", None),
                    EventKind::Unknown { raw } => ("Unknown", Some(raw.as_str())),
                };
                let expanded = matches!(&state.detail, DetailState::Expanded(i) if *i == re.index);
                let marker = if expanded {
                    "▼"
                } else if detail.is_some() {
                    "▶"
                } else {
                    " "
                };
                let line = match detail {
                    Some(d) => format!(
                        "  {:>3}  {:<22}{} {}",
                        re.index,
                        kind_str,
                        marker,
                        sanitize(d)
                    ),
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

fn render_detail(frame: &mut Frame, state: &AppState, loaded: Option<&LoadedSession>, area: Rect) {
    let content = match (&state.detail, loaded) {
        (DetailState::Expanded(index), Some(s)) => match s.replay.get(*index) {
            Some(re) => match &re.payload {
                Some(val) => {
                    let raw = serde_json::to_string_pretty(val).unwrap_or_else(|_| "?".into());
                    sanitize(&raw)
                }
                None => "(no payload)".into(),
            },
            None => "(event not found)".into(),
        },
        _ => String::new(),
    };

    let p = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Detail"))
        .wrap(Wrap { trim: false });

    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn sanitize_strips_csi_color_sequence() {
        assert_eq!(sanitize("\x1b[0;31mred\x1b[0m"), "red");
    }

    #[test]
    fn sanitize_strips_bare_csi_reset() {
        assert_eq!(sanitize("\x1b[m"), "");
    }

    #[test]
    fn sanitize_strips_two_char_escape() {
        // ESC M (reverse index) — should be consumed silently
        assert_eq!(sanitize("\x1bMtext"), "text");
    }

    #[test]
    fn sanitize_preserves_normal_text() {
        assert_eq!(sanitize("hello world"), "hello world");
    }

    #[test]
    fn sanitize_preserves_tab_and_newline() {
        assert_eq!(sanitize("a\tb\nc"), "a\tb\nc");
    }

    #[test]
    fn sanitize_replaces_other_control_chars() {
        // BEL (0x07) and BS (0x08) should become ·
        assert_eq!(sanitize("a\x07b\x08c"), "a·b·c");
    }

    #[test]
    fn sanitize_handles_bare_esc_at_end() {
        assert_eq!(sanitize("text\x1b"), "text");
    }

    #[test]
    fn sanitize_strips_cursor_movement_sequence() {
        // ESC[2J — erase display
        assert_eq!(sanitize("\x1b[2Jclear"), "clear");
    }
}
