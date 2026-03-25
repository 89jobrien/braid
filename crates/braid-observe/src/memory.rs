#[cfg(feature = "test-support")]
mod inner {
    use anyhow::Result;
    use braid_model::{Event, SessionId};
    use braid_ports::SessionStorage;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory implementation of SessionStorage for use in tests.
    #[derive(Default)]
    pub struct InMemorySessionStorage {
        sessions: Mutex<HashMap<String, Vec<Event>>>,
    }

    impl InMemorySessionStorage {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl SessionStorage for InMemorySessionStorage {
        fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
            self.sessions
                .lock()
                .unwrap()
                .insert(id.0.clone(), events.to_vec());
            Ok(())
        }

        fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
            self.sessions
                .lock()
                .unwrap()
                .get(&id.0)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("session not found: {}", id.0))
        }

        fn list(&self) -> Result<Vec<SessionId>> {
            let map = self.sessions.lock().unwrap();
            let mut ids: Vec<SessionId> = map.keys().map(|k| SessionId(k.clone())).collect();
            ids.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(ids)
        }

        fn prune(&self, keep: usize) -> Result<usize> {
            let mut map = self.sessions.lock().unwrap();
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            if keys.len() <= keep {
                return Ok(0);
            }
            let to_delete = keys.len() - keep;
            for key in keys.iter().take(to_delete) {
                map.remove(key);
            }
            Ok(to_delete)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use braid_model::{Event, EventKind, SessionId};

        fn evt(id: &str) -> Event {
            Event {
                session_id: SessionId(id.into()),
                kind: EventKind::SessionStarted,
            }
        }

        #[test]
        fn in_memory_write_and_load() {
            let store = InMemorySessionStorage::new();
            let id = SessionId("s1".into());
            store.write(&id, &[evt("s1")]).unwrap();
            let loaded = store.load(&id).unwrap();
            assert_eq!(loaded.len(), 1);
        }

        #[test]
        fn in_memory_load_missing_errors() {
            let store = InMemorySessionStorage::new();
            let err = store.load(&SessionId("ghost".into())).unwrap_err();
            assert!(err.to_string().contains("ghost"));
        }
    }
}

#[cfg(feature = "test-support")]
pub use inner::InMemorySessionStorage;
