use std::io::Write;

use anyhow::Result;
use braid_model::{Event, EventKind};

use crate::store::SessionMeta;

pub fn render_session(
    events: &[Event],
    meta: Option<&SessionMeta>,
    out: &mut impl Write,
) -> Result<()> {
    let event_count = events.len();
    match meta {
        Some(m) => {
            // "2026-03-24T05:00:00Z" → "2026-03-24 05:00:00 UTC"
            let ts = m
                .written_at
                .replace('T', " ")
                .trim_end_matches('Z')
                .to_string()
                + " UTC";
            writeln!(
                out,
                "Session: {}  ({})  {} events",
                m.session_id.0, ts, event_count
            )?;
        }
        None => {
            let sid = events
                .first()
                .map(|e| e.session_id.0.as_str())
                .unwrap_or("unknown");
            writeln!(out, "Session: {}  {} events", sid, event_count)?;
        }
    }

    // Separator: ASCII hyphens, 50 chars wide
    writeln!(out, "{}", "-".repeat(50))?;

    // Event rows: right-aligned index (2 chars), kind (20 chars), optional detail
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

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{Event, EventKind, SessionId};

    fn all_event_kinds(session_id: &str) -> Vec<Event> {
        vec![
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::SessionStarted,
            },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::ProviderResponded,
            },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::ToolCalled {
                    tool_name: "echo".into(),
                },
            },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::ToolCompleted {
                    tool_name: "echo".into(),
                },
            },
            Event {
                session_id: SessionId(session_id.into()),
                kind: EventKind::SessionCompleted,
            },
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
        assert!(
            s.contains("ProviderResponded"),
            "should list ProviderResponded"
        );
        assert!(s.contains("ToolCalled"), "should list ToolCalled");
        assert!(s.contains("echo"), "should show tool name");
        assert!(s.contains("ToolCompleted"), "should list ToolCompleted");
        assert!(
            s.contains("SessionCompleted"),
            "should list SessionCompleted"
        );
        assert!(s.contains("  1 "), "should have index 1");
        assert!(s.contains("  5 "), "should have index 5");
    }

    #[test]
    fn renders_gracefully_without_meta() {
        let events = all_event_kinds("xyz");
        let mut out = Vec::new();
        render_session(&events, None, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("SessionStarted"));
        assert!(
            !s.contains("2026-"),
            "should not have timestamp when meta absent"
        );
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
        let sep_line = s.lines().find(|l| l.starts_with('-')).unwrap();
        assert!(sep_line.is_ascii(), "separator must be ASCII only");
    }
}
