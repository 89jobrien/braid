use braid_model::Role;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::catalog::Catalog;
use crate::keys::{AppMode, AppState, DetailState, Focus, HarnessFocus};
use crate::model::{AppModel, LoadedSession};

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

pub fn render(
    frame: &mut Frame,
    state: &AppState,
    loaded: Option<&LoadedSession>,
    model: &AppModel,
) {
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

    match state.mode {
        AppMode::Inspect => {
            let status = Paragraph::new(Line::from(vec![
                Span::styled(
                    "braid inspect",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    "Tab: focus  ↑↓: nav  Enter: expand  r: reload  F1: harness  q: quit",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            frame.render_widget(status, chunks[0]);

            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
                .split(chunks[1]);

            render_session_list(frame, state, cols[0]);
            render_right_column(frame, state, loaded, cols[1]);
        }
        AppMode::Harness => {
            let status = Paragraph::new(Line::from(vec![
                Span::styled(
                    "braid harness",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    "Tab: focus  Enter: send  Ctrl-q: quit  F1: inspect",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            frame.render_widget(status, chunks[0]);
            render_harness(frame, state, model, chunks[1]);
        }
    }
}

fn render_harness(frame: &mut Frame, state: &AppState, model: &AppModel, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(55),
            Constraint::Percentage(25),
        ])
        .split(area);

    render_catalog(frame, state, &model.catalog, cols[0]);
    render_chat(frame, state, model, cols[1]);
    render_context(frame, state, model, cols[2]);
}

fn render_catalog(frame: &mut Frame, state: &AppState, catalog: &Catalog, area: Rect) {
    let focused = state.harness_focus == HarnessFocus::Catalog;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let visible = catalog.visible();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|e| {
            let line = format!("[{}] {}", e.kind.label(), e.name);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Components"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ListState::default();
    if !visible.is_empty() {
        list_state.select(Some(catalog.cursor));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_chat(frame: &mut Frame, state: &AppState, model: &AppModel, area: Rect) {
    let focused = state.harness_focus == HarnessFocus::Chat;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    // Split: scrollable history above, input box below
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    // Inner width available for text (subtract 2 for borders, 2 for indent).
    let inner_w = rows[0].width.saturating_sub(2) as usize;
    let text_w = inner_w.saturating_sub(2).max(10);

    // Build message lines with manual word-wrap so scroll counting stays accurate.
    let mut lines: Vec<Line> = Vec::new();
    for msg in &model.chat_messages {
        let (label, style) = match msg.role {
            Role::User => (
                "you",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Role::Assistant => (
                "braid",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            _ => ("sys", Style::default().fg(Color::DarkGray)),
        };
        lines.push(Line::from(Span::styled(format!("{label}:"), style)));
        for text_line in msg.text.lines() {
            for wrapped in word_wrap(text_line, text_w) {
                lines.push(Line::from(format!("  {wrapped}")));
            }
        }
        lines.push(Line::from(""));
    }

    if model.thinking {
        lines.push(Line::from(Span::styled(
            "braid: thinking...",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::DIM),
        )));
    }

    // Scroll: show tail of messages
    let total = lines.len();
    let height = rows[0].height.saturating_sub(2) as usize;
    let skip = total.saturating_sub(height);
    let visible_lines: Vec<Line> = lines.into_iter().skip(skip).collect();

    let history = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("Chat"),
    );
    frame.render_widget(history, rows[0]);

    // Input box
    {
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = input_block.inner(rows[1]);
        frame.render_widget(input_block, rows[1]);
        frame.render_widget(&model.input, inner);
    }

    // Completion popup anchored above the input box
    if let Some(ref comp) = model.completion
        && !comp.items.is_empty()
    {
        let items: Vec<ListItem> = comp
            .items
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == comp.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}", comp.trigger.sigil(), name)).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(comp.trigger.title()),
        );

        let popup_h = u16::try_from(comp.items.len().min(8) + 2).unwrap_or(10);
        let popup_w = (rows[1].width / 2).max(30);
        let popup_area = Rect {
            x: rows[1].x + 1,
            y: rows[1].y.saturating_sub(popup_h),
            width: popup_w,
            height: popup_h,
        };

        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(list, popup_area);
    }
}

fn render_context(frame: &mut Frame, state: &AppState, model: &AppModel, area: Rect) {
    let focused = state.harness_focus == HarnessFocus::Context;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let msg_count = model.chat_messages.len();
    let status = if model.thinking {
        "engine: running".into()
    } else if msg_count == 0 {
        "no messages yet".into()
    } else {
        format!("{msg_count} message(s)")
    };

    let content = format!("session\n  {status}\n\nsources\n  doob\n  repo");

    let p = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Context"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
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

/// Word-wrap `text` to at most `width` columns, splitting on whitespace.
/// Returns at least one element (empty string for empty input).
fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            // Word itself longer than width — hard-break it
            if word.len() > width {
                for chunk in word.as_bytes().chunks(width) {
                    lines.push(String::from_utf8_lossy(chunk).into_owned());
                }
            } else {
                current.push_str(word);
            }
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            if word.len() > width {
                for chunk in word.as_bytes().chunks(width) {
                    lines.push(String::from_utf8_lossy(chunk).into_owned());
                }
            } else {
                current.push_str(word);
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{sanitize, word_wrap};

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

    #[test]
    fn word_wrap_fits_within_width() {
        let lines = word_wrap("hello world foo bar", 10);
        for l in &lines {
            assert!(l.len() <= 10, "line too long: {l:?}");
        }
        assert_eq!(lines.join(" "), "hello world foo bar");
    }

    #[test]
    fn word_wrap_hard_breaks_long_word() {
        let lines = word_wrap("abcdefghij", 4);
        assert_eq!(lines, ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn word_wrap_empty_string() {
        assert_eq!(word_wrap("", 40), [""]);
    }
}
