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

fn render_detail(frame: &mut Frame, state: &AppState, loaded: Option<&LoadedSession>, area: Rect) {
    let content = match (&state.detail, loaded) {
        (DetailState::Expanded(index), Some(s)) => match s.replay.get(*index) {
            Some(re) => match &re.payload {
                Some(val) => serde_json::to_string_pretty(val).unwrap_or_else(|_| "?".into()),
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
