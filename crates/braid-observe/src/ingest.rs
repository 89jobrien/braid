use std::path::Path;

use anyhow::Result;
use braid_model::{Event, EventKind, SessionId};

use crate::store::SessionStore;

/// Port: ingest events from an external source into the store.
pub trait Ingester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId>;
}

/// Adapter: ingest braid-native JSONL (already normalized).
pub struct BraidIngester;

impl Ingester for BraidIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<Event>(&line) {
                events.push(event);
            }
            // Skip lines that don't parse — forward compat
        }

        if events.is_empty() {
            anyhow::bail!("no events found in {}", source.display());
        }

        let id = events[0].session_id.clone();
        store.write(&id, &events)?;
        Ok(id)
    }
}

/// Adapter: ingest Claude Code conversation JSONL.
pub struct ClaudeCodeIngester;

impl Ingester for ClaudeCodeIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);
        let mut session_id: Option<SessionId> = None;
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };

            if session_id.is_none()
                && let Some(sid) = val.get("session_id").and_then(|v| v.as_str())
            {
                session_id = Some(SessionId(sid.to_owned()));
            }

            let sid = match &session_id {
                Some(s) => s.clone(),
                None => continue,
            };

            let kind = match val.get("type").and_then(|t| t.as_str()) {
                Some("summary") => EventKind::SessionStarted,
                Some("assistant") => EventKind::ProviderResponded,
                Some("tool_use") => {
                    let tool_name = val
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCalled { tool_name }
                }
                Some("tool_result") => {
                    let tool_name = val
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCompleted { tool_name }
                }
                _ => continue,
            };

            events.push(Event {
                session_id: sid,
                kind,
            });
        }

        let id = session_id
            .ok_or_else(|| anyhow::anyhow!("no session_id found in {}", source.display()))?;

        if !matches!(
            events.last().map(|e| &e.kind),
            Some(EventKind::SessionCompleted)
        ) {
            events.push(Event {
                session_id: id.clone(),
                kind: EventKind::SessionCompleted,
            });
        }

        store.write(&id, &events)?;
        Ok(id)
    }
}

/// Adapter: ingest devloop run transcript JSONL.
pub struct DevloopIngester;

impl Ingester for DevloopIngester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId> {
        use std::io::BufRead;

        let file = std::fs::File::open(source)?;
        let reader = std::io::BufReader::new(file);
        let mut session_id: Option<SessionId> = None;
        let mut events: Vec<Event> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };

            if session_id.is_none()
                && let Some(rid) = val.get("run_id").and_then(|v| v.as_str())
            {
                session_id = Some(SessionId(format!("devloop-{}", rid)));
            }

            let sid = match &session_id {
                Some(s) => s.clone(),
                None => continue,
            };

            let kind = match val.get("event").and_then(|t| t.as_str()) {
                Some("run_started") => EventKind::SessionStarted,
                Some("llm_response") => EventKind::ProviderResponded,
                Some("tool_call") => {
                    let tool_name = val
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCalled { tool_name }
                }
                Some("tool_result") => {
                    let tool_name = val
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    EventKind::ToolCompleted { tool_name }
                }
                Some("run_completed") => EventKind::SessionCompleted,
                _ => continue,
            };

            events.push(Event {
                session_id: sid,
                kind,
            });
        }

        let id =
            session_id.ok_or_else(|| anyhow::anyhow!("no run_id found in {}", source.display()))?;
        store.write(&id, &events)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let manifest = env!("CARGO_MANIFEST_DIR");
        std::path::PathBuf::from(manifest)
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn braid_ingester_loads_native_jsonl() {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path("braid-native.jsonl");

        let id = BraidIngester.ingest(&source, &store).unwrap();

        let events = store.load(&id).unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        assert_eq!(events[4].kind, EventKind::SessionCompleted);
    }

    #[test]
    fn claude_code_ingester_normalizes_conversation() {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path("claude-code.jsonl");

        let id = ClaudeCodeIngester.ingest(&source, &store).unwrap();

        let events = store.load(&id).unwrap();
        assert!(events.len() >= 3, "expected at least 3 normalized events");
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        let has_tool_called = events.iter().any(
            |e| matches!(&e.kind, EventKind::ToolCalled { tool_name } if tool_name == "read_file"),
        );
        assert!(has_tool_called, "expected ToolCalled for read_file");
    }

    /// Forward-compat round-trip: EventKind::Unknown must survive ingestion → storage → replay
    /// without dropping any fields. BraidIngester accepts native JSONL including the Unknown
    /// variant; ReplaySession must surface it with its payload intact.
    #[test]
    fn unknown_event_kind_round_trips_through_ingestion_and_replay() {
        use crate::replay::ReplaySession;
        use braid_model::EventKind;
        use std::io::Write;

        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        // Write a JSONL file that mixes known events with an Unknown variant.
        // EventKind::Unknown serializes as {"Unknown":{"raw":"<payload>"}}.
        let jsonl_path = dir.path().join("mixed.jsonl");
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, r#"{{"session_id":"unk-1","kind":"SessionStarted"}}"#).unwrap();
        writeln!(
            f,
            r#"{{"session_id":"unk-1","kind":{{"Unknown":{{"raw":"future-payload-data"}}}}}}"#
        )
        .unwrap();
        writeln!(f, r#"{{"session_id":"unk-1","kind":"SessionCompleted"}}"#).unwrap();

        // Ingest via BraidIngester (the native-JSONL path).
        let id = BraidIngester.ingest(&jsonl_path, &store).unwrap();
        assert_eq!(id.0, "unk-1");

        // Verify via store.load() — all three events present, Unknown preserved.
        let events = store.load(&id).unwrap();
        assert_eq!(events.len(), 3, "all three events must be loaded");
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        assert!(
            matches!(&events[1].kind, EventKind::Unknown { raw } if raw == "future-payload-data"),
            "Unknown event must preserve its raw field; got {:?}",
            events[1].kind
        );
        assert_eq!(events[2].kind, EventKind::SessionCompleted);

        // Verify via ReplaySession — same events, indexed 1-based, payload preserved.
        let replay = ReplaySession::load(&store, &id).unwrap();
        assert_eq!(replay.len(), 3, "ReplaySession must see all three events");

        let unknown_replay = replay.get(2).unwrap();
        assert!(
            matches!(&unknown_replay.event.kind, EventKind::Unknown { raw } if raw == "future-payload-data"),
            "ReplaySession must preserve Unknown event kind; got {:?}",
            unknown_replay.event.kind
        );

        // The raw JSON payload must also be present and contain the raw field value.
        let payload = unknown_replay
            .payload
            .as_ref()
            .expect("payload must be present");
        assert_eq!(
            payload["kind"]["Unknown"]["raw"], "future-payload-data",
            "ReplayEvent payload must preserve all fields of Unknown event"
        );
    }

    #[test]
    fn devloop_ingester_normalizes_run_transcript() {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path("devloop.jsonl");

        let id = DevloopIngester.ingest(&source, &store).unwrap();

        let events = store.load(&id).unwrap();
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        let has_tool = events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::ToolCalled { tool_name } if tool_name == "bash"));
        assert!(has_tool, "expected ToolCalled for bash");
        assert_eq!(events.last().unwrap().kind, EventKind::SessionCompleted);
    }
}
