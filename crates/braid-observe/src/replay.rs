use anyhow::Result;
use braid_model::{Event, SessionId};

use crate::store::SessionStore;

#[derive(Debug, Clone)]
pub struct ReplayEvent {
    pub index: usize, // 1-based, matching render output
    pub event: Event,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct ReplaySession {
    pub id: SessionId,
    events: Vec<ReplayEvent>,
}

impl ReplaySession {
    pub fn load(store: &SessionStore, id: &SessionId) -> Result<Self> {
        use std::io::BufRead;

        let root = store.root();
        let path = root.join(&id.0).join("events.jsonl");
        if !path.exists() {
            return Err(anyhow::anyhow!("session not found: {}", id.0));
        }

        let file = std::fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        let mut index = 0usize;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Event>(&line) else {
                continue;
            };
            let payload = serde_json::from_str::<serde_json::Value>(&line).ok();
            index += 1;
            events.push(ReplayEvent {
                index,
                event,
                payload,
            });
        }

        Ok(Self {
            id: id.clone(),
            events,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReplayEvent> {
        self.events.iter()
    }

    pub fn get(&self, index: usize) -> Option<&ReplayEvent> {
        // index is 1-based; 0 is out of range
        if index == 0 {
            return None;
        }
        self.events.get(index - 1)
    }

    pub const fn len(&self) -> usize {
        self.events.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionStore;
    use braid_model::{EventKind, SessionId};

    fn make_store_with_session(session_id: &str) -> (tempfile::TempDir, SessionStore, SessionId) {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let id = SessionId(session_id.into());
        let events = vec![
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionStarted,
            },
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::ToolCalled {
                    tool_name: "echo".into(),
                },
            },
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionCompleted,
            },
        ];
        store.write(&id, &events).expect("should succeed");
        (dir, store, id)
    }

    #[test]
    fn load_returns_indexed_events() {
        let (_dir, store, id) = make_store_with_session("r1");
        let replay = ReplaySession::load(&store, &id).expect("should succeed");
        assert_eq!(replay.len(), 3);
        assert_eq!(replay.get(1).expect("should succeed").index, 1);
        assert_eq!(
            replay.get(1).expect("should succeed").event.kind,
            EventKind::SessionStarted
        );
        assert_eq!(replay.get(3).expect("should succeed").index, 3);
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let (_dir, store, id) = make_store_with_session("r2");
        let replay = ReplaySession::load(&store, &id).expect("should succeed");
        assert!(replay.get(0).is_none(), "index 0 is out of range (1-based)");
        assert!(replay.get(99).is_none());
    }

    #[test]
    fn iter_yields_all_events_in_order() {
        let (_dir, store, id) = make_store_with_session("r3");
        let replay = ReplaySession::load(&store, &id).expect("should succeed");
        let kinds: Vec<_> = replay.iter().map(|e| &e.event.kind).collect();
        assert_eq!(kinds[0], &EventKind::SessionStarted);
        assert_eq!(kinds[2], &EventKind::SessionCompleted);
    }

    #[test]
    fn payload_is_preserved_from_jsonl() {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let id = SessionId("r4".into());

        let sess_dir = dir.path().join("r4");
        std::fs::create_dir_all(&sess_dir).expect("should succeed");
        std::fs::write(
            sess_dir.join("events.jsonl"),
            r#"{"session_id":"r4","kind":"SessionStarted"}
{"session_id":"r4","kind":{"ToolCalled":{"tool_name":"echo"}}}
"#,
        )
        .expect("should succeed");

        let replay = ReplaySession::load(&store, &id).expect("should succeed");
        let tool_event = replay.get(2).expect("should succeed");
        let payload = tool_event.payload.as_ref().expect("should succeed");
        assert_eq!(payload["kind"]["ToolCalled"]["tool_name"], "echo");
    }

    // --- Replay tests via SessionWriter ---

    /// Write events through `SessionWriter`, then load via `ReplaySession` and
    /// verify the round-trip: same count, same order, same EventKind values.
    #[test]
    fn session_writer_roundtrips_via_replay() {
        use crate::store::SessionWriter;
        use braid_model::EventKind;

        let dir = tempfile::tempdir().expect("should succeed");
        let id = SessionId("rw-1".into());

        let expected_kinds = [
            EventKind::SessionStarted,
            EventKind::ProviderResponded,
            EventKind::ToolCalled {
                tool_name: "grep".into(),
            },
            EventKind::ToolCompleted {
                tool_name: "grep".into(),
            },
            EventKind::SessionCompleted,
        ];

        // Write via SessionWriter
        let mut writer = SessionWriter::open(dir.path(), &id).expect("should succeed");
        for kind in &expected_kinds {
            writer
                .write_event(&braid_model::Event {
                    session_id: id.clone(),
                    kind: kind.clone(),
                })
                .expect("should succeed");
        }
        writer.finish().expect("should succeed");

        // Load via ReplaySession
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let replay = ReplaySession::load(&store, &id).expect("should succeed");

        assert_eq!(
            replay.len(),
            expected_kinds.len(),
            "replay length matches written event count"
        );

        for (i, expected_kind) in expected_kinds.iter().enumerate() {
            let re = replay.get(i + 1).expect("index should be in range");
            assert_eq!(re.index, i + 1, "replay index is 1-based and sequential");
            assert_eq!(
                &re.event.kind,
                expected_kind,
                "event kind at position {} round-trips correctly",
                i + 1
            );
            assert_eq!(
                &re.event.session_id,
                &id,
                "session_id is preserved at position {}",
                i + 1
            );
        }
    }

    /// Events written incrementally (one at a time) are visible through
    /// `ReplaySession` before `finish()` is called, matching partial-read semantics.
    #[test]
    fn session_writer_partial_replay_before_finish() {
        use crate::store::SessionWriter;
        use braid_model::EventKind;

        let dir = tempfile::tempdir().expect("should succeed");
        let id = SessionId("rw-2".into());
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");

        let mut writer = SessionWriter::open(dir.path(), &id).expect("should succeed");

        // Write first event; replay should see exactly one event immediately.
        writer
            .write_event(&braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionStarted,
            })
            .expect("should succeed");

        let partial = ReplaySession::load(&store, &id).expect("should succeed");
        assert_eq!(partial.len(), 1, "one event visible before finish");
        assert_eq!(
            partial.get(1).expect("should succeed").event.kind,
            EventKind::SessionStarted
        );

        // Write second event; replay now sees two.
        writer
            .write_event(&braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionCompleted,
            })
            .expect("should succeed");
        writer.finish().expect("should succeed");

        let full = ReplaySession::load(&store, &id).expect("should succeed");
        assert_eq!(full.len(), 2, "both events visible after finish");
        assert_eq!(
            full.get(2).expect("should succeed").event.kind,
            EventKind::SessionCompleted
        );
    }

    /// `ReplaySession` correctly reads a session written via `SessionStore::write`
    /// (batch path) rather than `SessionWriter` (streaming path).
    #[test]
    fn replay_loads_batch_written_session() {
        use braid_model::EventKind;

        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let id = SessionId("rw-3".into());

        let events = vec![
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionStarted,
            },
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::ToolCalled {
                    tool_name: "bash".into(),
                },
            },
            braid_model::Event {
                session_id: id.clone(),
                kind: EventKind::SessionCompleted,
            },
        ];

        store.write(&id, &events).expect("should succeed");

        let replay = ReplaySession::load(&store, &id).expect("should succeed");
        assert_eq!(replay.len(), events.len());

        // Verify each event in order
        for (i, original) in events.iter().enumerate() {
            let re = replay.get(i + 1).expect("should succeed");
            assert_eq!(&re.event, original, "event at index {} matches", i + 1);
        }
    }

    /// `ReplaySession` loaded from a missing session returns an error.
    #[test]
    fn replay_missing_session_returns_error() {
        let dir = tempfile::tempdir().expect("should succeed");
        let store = SessionStore::open(dir.path().to_path_buf()).expect("should succeed");
        let err = ReplaySession::load(&store, &SessionId("ghost".into()))
            .expect_err("should fail for missing session");
        assert!(err.to_string().contains("ghost"));
    }
}
