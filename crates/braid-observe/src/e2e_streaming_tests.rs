/// End-to-end test: Engine events are persisted in order via SessionWriter and
/// can be replayed losslessly through ReplaySession.
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use braid_core::{Engine, Provider, RunInput, SimpleLoopPlanner, StaticTool};
    use braid_model::{
        ContentPart, EventKind, Message, ProviderRequest, ProviderResponse, Role, SessionId,
    };

    use crate::{
        replay::ReplaySession,
        store::{SessionStore, SessionWriter},
    };

    // ---------------------------------------------------------------------------
    // Minimal providers
    // ---------------------------------------------------------------------------

    /// Returns a single text response with no tool calls.
    struct TextProvider {
        response: String,
    }

    impl Provider for TextProvider {
        fn complete(&self, _req: ProviderRequest) -> anyhow::Result<ProviderResponse> {
            Ok(ProviderResponse {
                message: Message {
                    role: Role::Assistant,
                    content: vec![ContentPart::Text {
                        text: self.response.clone(),
                    }],
                },
                token_count: None,
            })
        }
    }

    /// First call returns a ToolUse; second call returns text.
    struct ToolThenTextProvider {
        call_count: std::cell::Cell<u32>,
        tool_name: String,
        final_text: String,
    }

    impl ToolThenTextProvider {
        fn new(tool_name: impl Into<String>, final_text: impl Into<String>) -> Self {
            Self {
                call_count: std::cell::Cell::new(0),
                tool_name: tool_name.into(),
                final_text: final_text.into(),
            }
        }
    }

    impl Provider for ToolThenTextProvider {
        fn complete(&self, _req: ProviderRequest) -> anyhow::Result<ProviderResponse> {
            let n = self.call_count.get();
            self.call_count.set(n + 1);
            if n == 0 {
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::ToolUse {
                            id: "call-1".into(),
                            name: self.tool_name.clone(),
                            input: serde_json::json!({"arg": "value"}),
                        }],
                    },
                    token_count: None,
                })
            } else {
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::Text {
                            text: self.final_text.clone(),
                        }],
                    },
                    token_count: None,
                })
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Helper
    // ---------------------------------------------------------------------------

    fn user_msg(text: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: text.into() }],
        }
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    /// Simplest case: a single-turn session (no tools) is persisted and replayed
    /// with events in the correct order and correct count.
    #[test]
    fn simple_session_events_persisted_and_replayed_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let session_id = SessionId("e2e-simple".into());

        // Wire: Engine → event_callback → SessionWriter
        let writer_arc: Arc<Mutex<SessionWriter>> = Arc::new(Mutex::new(
            SessionWriter::open(dir.path(), &session_id).unwrap(),
        ));

        let writer_cb = Arc::clone(&writer_arc);
        let engine = Engine::new(
            StaticTool::new("unused", "unused-output"),
            TextProvider {
                response: "hello back".into(),
            },
        )
        .with_event_callback(move |event| {
            writer_cb
                .lock()
                .unwrap()
                .write_event(event)
                .expect("write_event failed");
        });

        let output = engine
            .run(
                RunInput {
                    session_id: session_id.clone(),
                    messages: vec![user_msg("hello")],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        // Drop engine so the Arc inside the callback is released before try_unwrap.
        drop(engine);

        // Finish the writer to flush meta.json
        Arc::try_unwrap(writer_arc)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap()
            .finish()
            .unwrap();

        // Engine returned 3 events in RunOutput
        assert_eq!(output.events.len(), 3);

        // Load via SessionStore + ReplaySession
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let replay = ReplaySession::load(&store, &session_id).unwrap();

        // Order: SessionStarted → ProviderResponded → SessionCompleted
        assert_eq!(replay.len(), 3, "all three events must be persisted");

        let e1 = replay.get(1).unwrap();
        let e2 = replay.get(2).unwrap();
        let e3 = replay.get(3).unwrap();

        assert_eq!(e1.index, 1);
        assert!(
            matches!(e1.event.kind, EventKind::SessionStarted),
            "first event must be SessionStarted, got {:?}",
            e1.event.kind
        );
        assert!(
            matches!(e2.event.kind, EventKind::ProviderResponded),
            "second event must be ProviderResponded, got {:?}",
            e2.event.kind
        );
        assert!(
            matches!(e3.event.kind, EventKind::SessionCompleted),
            "third event must be SessionCompleted, got {:?}",
            e3.event.kind
        );

        // Payloads are preserved (JSON round-trip)
        assert!(e1.payload.is_some(), "payload must not be None");
        assert_eq!(e1.payload.as_ref().unwrap()["session_id"], "e2e-simple");
    }

    /// A session with a tool call round-trip: verifies ToolCalled / ToolCompleted
    /// events appear in the persisted log and that tool_name payload is intact.
    #[test]
    fn tool_call_events_persisted_with_correct_tool_name() {
        let dir = tempfile::tempdir().unwrap();
        let session_id = SessionId("e2e-tool".into());

        let writer_arc: Arc<Mutex<SessionWriter>> = Arc::new(Mutex::new(
            SessionWriter::open(dir.path(), &session_id).unwrap(),
        ));

        let writer_cb = Arc::clone(&writer_arc);
        let engine = Engine::new(
            StaticTool::new("my_tool", "tool-result"),
            ToolThenTextProvider::new("my_tool", "done"),
        )
        .with_event_callback(move |event| {
            writer_cb
                .lock()
                .unwrap()
                .write_event(event)
                .expect("write_event failed");
        });

        let output = engine
            .run(
                RunInput {
                    session_id: session_id.clone(),
                    messages: vec![user_msg("use a tool")],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        drop(engine);
        Arc::try_unwrap(writer_arc)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap()
            .finish()
            .unwrap();

        // Expected: SessionStarted, ProviderResponded(1), ToolCalled, ToolCompleted,
        //           ProviderResponded(2), SessionCompleted — 6 events
        assert_eq!(output.events.len(), 6);

        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let replay = ReplaySession::load(&store, &session_id).unwrap();

        assert_eq!(replay.len(), 6, "all 6 events must be persisted");

        // Verify ordering
        assert!(matches!(
            replay.get(1).unwrap().event.kind,
            EventKind::SessionStarted
        ));
        assert!(matches!(
            replay.get(2).unwrap().event.kind,
            EventKind::ProviderResponded
        ));
        let tool_called = &replay.get(3).unwrap().event.kind;
        assert!(
            matches!(tool_called, EventKind::ToolCalled { tool_name } if tool_name == "my_tool"),
            "expected ToolCalled(my_tool), got {tool_called:?}"
        );
        let tool_completed = &replay.get(4).unwrap().event.kind;
        assert!(
            matches!(tool_completed, EventKind::ToolCompleted { tool_name } if tool_name == "my_tool"),
            "expected ToolCompleted(my_tool), got {tool_completed:?}"
        );
        assert!(matches!(
            replay.get(5).unwrap().event.kind,
            EventKind::ProviderResponded
        ));
        assert!(matches!(
            replay.get(6).unwrap().event.kind,
            EventKind::SessionCompleted
        ));

        // Payload losslessness: tool_name survives JSON round-trip
        let payload = replay.get(3).unwrap().payload.as_ref().unwrap();
        assert_eq!(
            payload["kind"]["ToolCalled"]["tool_name"], "my_tool",
            "tool_name must survive JSON round-trip in payload"
        );
    }

    /// Verifies that events emitted by Engine match exactly what ReplaySession
    /// returns — lossless round-trip check.
    #[test]
    fn engine_events_match_replayed_events_losslessly() {
        let dir = tempfile::tempdir().unwrap();
        let session_id = SessionId("e2e-lossless".into());

        let writer_arc: Arc<Mutex<SessionWriter>> = Arc::new(Mutex::new(
            SessionWriter::open(dir.path(), &session_id).unwrap(),
        ));

        let writer_cb = Arc::clone(&writer_arc);
        let engine = Engine::new(
            StaticTool::new("unused", "result"),
            TextProvider {
                response: "lossless check".into(),
            },
        )
        .with_event_callback(move |event| {
            writer_cb
                .lock()
                .unwrap()
                .write_event(event)
                .expect("write_event failed");
        });

        let output = engine
            .run(
                RunInput {
                    session_id: session_id.clone(),
                    messages: vec![user_msg("check lossless")],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        drop(engine);
        Arc::try_unwrap(writer_arc)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap()
            .finish()
            .unwrap();

        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let replay = ReplaySession::load(&store, &session_id).unwrap();

        // Every event in RunOutput must appear in ReplaySession at the same position.
        assert_eq!(
            replay.len(),
            output.events.len(),
            "replayed event count must match engine output"
        );

        for (i, engine_event) in output.events.iter().enumerate() {
            let replayed = replay.get(i + 1).unwrap();
            assert_eq!(
                replayed.event.kind,
                engine_event.kind,
                "event at position {} kind mismatch",
                i + 1
            );
            assert_eq!(
                replayed.event.session_id,
                engine_event.session_id,
                "session_id mismatch at position {}",
                i + 1
            );
        }
    }
}
