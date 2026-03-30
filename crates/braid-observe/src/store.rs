use anyhow::Result;
use braid_model::{Event, SessionId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: SessionId,
    pub written_at: String, // RFC 3339
    pub event_count: usize,
}

pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        let dir = self.session_dir(id);
        fs::create_dir_all(&dir)?;

        // Write events.jsonl
        let events_path = dir.join("events.jsonl");
        let mut f = fs::File::create(&events_path)?;
        for event in events {
            let line = serde_json::to_string(event)?;
            writeln!(f, "{}", line)?;
        }

        // Write meta.json atomically (write-then-rename)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let written_at = format_rfc3339(now.as_secs());

        let meta = SessionMeta {
            session_id: id.clone(),
            written_at,
            event_count: events.len(),
        };
        let meta_json = serde_json::to_string(&meta)?;
        let tmp_path = dir.join("meta.json.tmp");
        fs::write(&tmp_path, &meta_json)?;
        fs::rename(&tmp_path, dir.join("meta.json"))?;

        Ok(())
    }

    pub fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        let path = self.session_dir(id).join("events.jsonl");
        if !path.exists() {
            return Err(anyhow::anyhow!("session not found: {}", id.0));
        }
        let file = fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            // Skip lines we can't parse — forward-compat when new EventKind variants
            // are added in future versions.  Unknown lines are silently dropped.
            if let Ok(event) = serde_json::from_str::<Event>(&line) {
                events.push(event);
            }
        }
        Ok(events)
    }

    pub fn load_meta(&self, id: &SessionId) -> Result<Option<SessionMeta>> {
        let path = self.session_dir(id).join("meta.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let meta: SessionMeta = serde_json::from_str(&content)?;
        Ok(Some(meta))
    }

    pub fn list(&self) -> Result<Vec<SessionId>> {
        let mut entries: Vec<(SessionId, String)> = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = SessionId(entry.file_name().to_string_lossy().into_owned());
            let written_at = self
                .load_meta(&id)?
                .map(|m| m.written_at)
                .unwrap_or_else(|| {
                    entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| format_rfc3339(d.as_secs()))
                        .unwrap_or_default()
                });
            entries.push((id, written_at));
        }
        // Sort newest first (RFC 3339 strings sort lexicographically)
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(entries.into_iter().map(|(id, _)| id).collect())
    }

    pub fn prune(&self, keep: usize) -> Result<usize> {
        let all = self.list()?;
        if all.len() <= keep {
            return Ok(0);
        }
        let to_delete = &all[keep..];
        let count = to_delete.len();
        for id in to_delete {
            let dir = self.session_dir(id);
            fs::remove_dir_all(&dir)?;
        }
        Ok(count)
    }

    fn session_dir(&self, id: &SessionId) -> PathBuf {
        self.root.join(&id.0)
    }
}

/// Format unix seconds as RFC 3339 (UTC, no sub-second, no external deps).
fn format_rfc3339(secs: u64) -> String {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{Event, EventKind, SessionId};
    use std::io::Write;

    fn make_events(session_id: &str) -> Vec<Event> {
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
                kind: EventKind::SessionCompleted,
            },
        ]
    }

    #[test]
    fn writes_and_loads_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let id = SessionId("sess-1".into());
        let events = make_events("sess-1");
        store.write(&id, &events).unwrap();
        let loaded = store.load(&id).unwrap();
        assert_eq!(loaded, events);
    }

    #[test]
    fn list_returns_sessions_by_recency() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        for id_str in &["sess-a", "sess-b", "sess-c"] {
            let id = SessionId(id_str.to_string());
            store.write(&id, &make_events(id_str)).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].0, "sess-c");
        assert_eq!(list[2].0, "sess-a");
    }

    #[test]
    fn prune_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        for i in 0..5 {
            let id = SessionId(format!("sess-{i}"));
            store
                .write(&id, &make_events(&format!("sess-{i}")))
                .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let deleted = store.prune(3).unwrap();
        assert_eq!(deleted, 2);

        let remaining = store.list().unwrap();
        assert_eq!(remaining.len(), 3);
        assert_eq!(remaining[0].0, "sess-4");
    }

    #[test]
    fn load_missing_session_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let err = store.load(&SessionId("ghost".into())).unwrap_err();
        assert!(err.to_string().contains("ghost"));
    }

    #[test]
    fn load_succeeds_when_meta_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

        // Write events.jsonl manually without meta.json
        let sess_dir = dir.path().join("orphan");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let id = SessionId("orphan".into());
        let events = make_events("orphan");
        let mut f = std::fs::File::create(sess_dir.join("events.jsonl")).unwrap();
        for e in &events {
            writeln!(f, "{}", serde_json::to_string(e).unwrap()).unwrap();
        }

        // load() should succeed (best-effort)
        let loaded = store.load(&id).unwrap();
        assert_eq!(loaded, events);

        // load_meta() returns None
        let meta = store.load_meta(&id).unwrap();
        assert!(meta.is_none());
    }

    #[test]
    fn prune_keep_all_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        for i in 0..3 {
            let id = SessionId(format!("s{i}"));
            store.write(&id, &make_events(&format!("s{i}"))).unwrap();
        }
        let deleted = store.prune(10).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(store.list().unwrap().len(), 3);
    }

    /// Golden test: these JSON strings ARE the on-disk format.
    /// If serde serialization of `Event` / `EventKind` changes, this test breaks
    /// deliberately — update the strings AND ensure backward compatibility.
    #[test]
    fn event_json_format_is_stable() {
        use braid_model::EventKind;
        let cases: &[(&str, EventKind)] = &[
            (
                r#"{"session_id":"s","kind":"SessionStarted"}"#,
                EventKind::SessionStarted,
            ),
            (
                r#"{"session_id":"s","kind":"ProviderResponded"}"#,
                EventKind::ProviderResponded,
            ),
            (
                r#"{"session_id":"s","kind":{"ToolCalled":{"tool_name":"echo"}}}"#,
                EventKind::ToolCalled {
                    tool_name: "echo".into(),
                },
            ),
            (
                r#"{"session_id":"s","kind":{"ToolCompleted":{"tool_name":"echo"}}}"#,
                EventKind::ToolCompleted {
                    tool_name: "echo".into(),
                },
            ),
            (
                r#"{"session_id":"s","kind":"SessionCompleted"}"#,
                EventKind::SessionCompleted,
            ),
        ];
        for (json, expected_kind) in cases {
            let event: Event = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("failed to parse {json}: {e}"));
            assert_eq!(&event.kind, expected_kind, "kind mismatch for {json}");
            let re = serde_json::to_string(&event).unwrap();
            assert_eq!(
                &re, json,
                "re-serialized form changed for {expected_kind:?}"
            );
        }
    }

    /// Forward compatibility: a JSONL containing an unrecognized event kind
    /// (from a future version) must load without error, silently skipping
    /// the unknown lines.
    #[test]
    fn forward_compat_skips_unknown_event_kind() {
        let dir = tempfile::tempdir().unwrap();
        let sess_dir = dir.path().join("s1");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let mut f = std::fs::File::create(sess_dir.join("events.jsonl")).unwrap();
        writeln!(f, r#"{{"session_id":"s1","kind":"SessionStarted"}}"#).unwrap();
        // Simulate an event kind added in a future release
        writeln!(
            f,
            r#"{{"session_id":"s1","kind":{{"ProviderStreaming":{{"token":"hi"}}}}}}"#
        )
        .unwrap();
        writeln!(f, r#"{{"session_id":"s1","kind":"SessionCompleted"}}"#).unwrap();

        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let events = store.load(&SessionId("s1".into())).unwrap();

        assert_eq!(events.len(), 2, "unknown variant must be skipped");
        assert_eq!(events[0].kind, EventKind::SessionStarted);
        assert_eq!(events[1].kind, EventKind::SessionCompleted);
    }

    #[test]
    fn unknown_event_kind_survives_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let sess_dir = dir.path().join("u1");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let mut f = std::fs::File::create(sess_dir.join("events.jsonl")).unwrap();
        writeln!(
            f,
            "{{\"session_id\":\"u1\",\"kind\":{{\"Unknown\":{{\"raw\":\"future-event\"}}}}}}"
        )
        .unwrap();

        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let events = store.load(&SessionId("u1".into())).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].kind, EventKind::Unknown { .. }));
    }
}
