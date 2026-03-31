use anyhow::Result;
use braid_model::{
    ContentPart, Event, EventKind, Message, ProviderRequest, ProviderResponse, Role, SessionId,
    ToolCall, ToolResult,
};

use crate::planner::{Action, Planner, SessionState};
use crate::tools::ToolExecutor;

const DEFAULT_MAX_TURNS: u32 = 10;

type Redactor = Box<dyn Fn(&Message) -> Message + Send + Sync + 'static>;
type EventCallback = Box<dyn Fn(&Event) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub max_turns: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub provider_response: ProviderResponse,
    pub events: Vec<Event>,
}

pub struct Engine<T, P> {
    tool_executor: T,
    provider: P,
    redactor: Option<Redactor>,
    event_callback: Option<EventCallback>,
}

impl<T, P> Engine<T, P> {
    pub fn new(tool_executor: T, provider: P) -> Self {
        Self {
            tool_executor,
            provider,
            redactor: None,
            event_callback: None,
        }
    }

    /// Attach a message redactor applied to all messages before each provider call.
    pub fn with_redactor(
        mut self,
        f: impl Fn(&Message) -> Message + Send + Sync + 'static,
    ) -> Self {
        self.redactor = Some(Box::new(f));
        self
    }

    /// Attach an event callback invoked for each event as it is emitted.
    pub fn with_event_callback(mut self, f: impl Fn(&Event) + Send + Sync + 'static) -> Self {
        self.event_callback = Some(Box::new(f));
        self
    }
}

pub trait Provider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

impl Provider for Box<dyn Provider> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}

fn extract_tool_calls(message: &Message) -> Vec<ToolCall> {
    message
        .content
        .iter()
        .filter_map(|part| match part {
            ContentPart::ToolUse { id, name, input } => Some(ToolCall {
                id: id.clone(),
                name: name.clone(),
                // Produces compact JSON string from serde_json::Value
                input: input.to_string(),
            }),
            _ => None,
        })
        .collect()
}

fn tool_result_to_message(call: &ToolCall, result: &ToolResult) -> Message {
    Message {
        role: Role::Tool,
        content: vec![ContentPart::ToolResult {
            tool_use_id: call.id.clone(),
            content: result.output.clone(),
        }],
    }
}

impl<T, P> Engine<T, P>
where
    T: ToolExecutor,
    P: Provider,
{
    pub fn run(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput> {
        let max_turns = input.max_turns.unwrap_or(DEFAULT_MAX_TURNS);
        let mut events = Vec::new();
        let mut state = SessionState {
            messages: input.messages,
            pending_tool_calls: vec![],
            last_provider_response: None,
            turn_count: 0,
            max_turns,
        };

        macro_rules! emit {
            ($event:expr) => {{
                let event = $event;
                if let Some(cb) = &self.event_callback {
                    cb(&event);
                }
                events.push(event);
            }};
        }

        emit!(Event {
            session_id: input.session_id.clone(),
            kind: EventKind::SessionStarted,
        });

        loop {
            let action = planner.next_action(&state)?;

            match action {
                Action::CallProvider { messages } => {
                    let messages = match &self.redactor {
                        Some(r) => messages.iter().map(r).collect(),
                        None => messages,
                    };
                    let response = self.provider.complete(ProviderRequest {
                        messages,
                        tools: vec![],
                    })?;

                    // Add assistant message to conversation
                    state.messages.push(response.message.clone());

                    // Extract tool calls from response
                    state.pending_tool_calls = extract_tool_calls(&response.message);
                    state.last_provider_response = Some(response);
                    state.turn_count += 1;

                    emit!(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ProviderResponded,
                    });
                }
                Action::ExecuteTool { call } => {
                    emit!(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCalled {
                            tool_name: call.name.clone(),
                        },
                    });

                    let result = self.tool_executor.execute(call.clone())?;

                    // Add tool result as message
                    state.messages.push(tool_result_to_message(&call, &result));

                    // Remove executed call from pending
                    state.pending_tool_calls.retain(|c| c.id != call.id);

                    emit!(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCompleted {
                            tool_name: call.name.clone(),
                        },
                    });
                }
                Action::Finish { response } => {
                    emit!(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::SessionCompleted,
                    });

                    return Ok(RunOutput {
                        provider_response: response,
                        events,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::SimpleLoopPlanner;
    use braid_model::ContentPart;

    struct TestProvider;

    impl Provider for TestProvider {
        fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
            let first_text = request
                .messages
                .iter()
                .flat_map(|m| &m.content)
                .find_map(|c| match c {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            Ok(ProviderResponse {
                message: Message {
                    role: Role::Assistant,
                    content: vec![ContentPart::Text {
                        text: format!("provider saw: {}", first_text),
                    }],
                },
                token_count: None,
            })
        }
    }

    /// Provider that returns a tool call on first request, text on second.
    struct ToolCallingProvider {
        call_count: std::cell::Cell<u32>,
    }

    impl ToolCallingProvider {
        fn new() -> Self {
            Self {
                call_count: std::cell::Cell::new(0),
            }
        }
    }

    impl Provider for ToolCallingProvider {
        fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);

            if count == 0 {
                // First call: request a tool
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::ToolUse {
                            id: "call_1".into(),
                            name: "echo".into(),
                            input: serde_json::json!({"text": "hello"}),
                        }],
                    },
                    token_count: None,
                })
            } else {
                // Second call: final text response
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::Text {
                            text: "done after tool".into(),
                        }],
                    },
                    token_count: None,
                })
            }
        }
    }

    #[test]
    fn runs_a_minimal_session() {
        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "tool output"),
            TestProvider,
        );
        let output = engine
            .run(
                RunInput {
                    session_id: SessionId("session-1".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "hello".into(),
                        }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let response_text = match &output.provider_response.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(response_text, "provider saw: hello");
        // SessionStarted, ProviderResponded, SessionCompleted
        assert_eq!(output.events.len(), 3);
        assert!(matches!(output.events[0].kind, EventKind::SessionStarted));
        assert!(matches!(
            output.events[1].kind,
            EventKind::ProviderResponded
        ));
        assert!(matches!(output.events[2].kind, EventKind::SessionCompleted));
    }

    #[test]
    fn runs_tool_call_loop() {
        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "echoed output"),
            ToolCallingProvider::new(),
        );
        let output = engine
            .run(
                RunInput {
                    session_id: SessionId("session-2".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "use a tool".into(),
                        }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let response_text = match &output.provider_response.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(response_text, "done after tool");
        // SessionStarted, ProviderResponded (1st), ToolCalled, ToolCompleted,
        // ProviderResponded (2nd), SessionCompleted
        assert_eq!(output.events.len(), 6);
        assert!(matches!(
            output.events[2].kind,
            EventKind::ToolCalled { .. }
        ));
        assert!(matches!(
            output.events[3].kind,
            EventKind::ToolCompleted { .. }
        ));
    }

    #[test]
    fn respects_max_turns() {
        // Provider always requests a tool — would loop forever without limit
        struct InfiniteToolProvider;
        impl Provider for InfiniteToolProvider {
            fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::ToolUse {
                            id: "call_1".into(),
                            name: "echo".into(),
                            input: serde_json::json!({}),
                        }],
                    },
                    token_count: None,
                })
            }
        }

        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "out"),
            InfiniteToolProvider,
        );
        let output = engine
            .run(
                RunInput {
                    session_id: SessionId("session-3".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text { text: "go".into() }],
                    }],
                    max_turns: Some(2),
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        // Should finish after 2 provider calls despite pending tools
        assert!(matches!(
            output.events.last().unwrap().kind,
            EventKind::SessionCompleted
        ));
    }

    #[test]
    fn with_redactor_transforms_messages_before_provider() {
        // Provider echoes back what it receives; redactor replaces "secret" with "[R]"
        struct EchoProvider;
        impl Provider for EchoProvider {
            fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
                let text = request
                    .messages
                    .iter()
                    .flat_map(|m| &m.content)
                    .find_map(|c| match c {
                        ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::Text {
                            text: format!("saw: {text}"),
                        }],
                    },
                    token_count: None,
                })
            }
        }

        let engine = Engine::new(crate::tools::StaticTool::new("echo", "out"), EchoProvider)
            .with_redactor(|msg| {
                let redacted_content = msg
                    .content
                    .iter()
                    .map(|part| match part {
                        ContentPart::Text { text } => ContentPart::Text {
                            text: text.replace("secret", "[R]"),
                        },
                        other => other.clone(),
                    })
                    .collect();
                Message {
                    role: msg.role.clone(),
                    content: redacted_content,
                }
            });

        let output = engine
            .run(
                RunInput {
                    session_id: SessionId("session-r".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "my secret key".into(),
                        }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let response_text = match &output.provider_response.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        // Provider should have seen "[R]" not "secret"
        assert!(response_text.contains("[R]"), "redactor should have fired");
        assert!(
            !response_text.contains("secret"),
            "raw secret should not reach provider"
        );
    }

    /// Port-boundary test: Engine accepts any `Fn(&Event)` as its event sink.
    /// The concrete `SessionStore` / `SessionWriter` types from `braid-observe`
    /// must never appear here. This test verifies that any type implementing
    /// `Fn(&Event)` can be wired in — enforcing the hexagonal boundary at the
    /// type level. If someone tries to add `braid-observe` as a dependency of
    /// `braid-core`, the `Cargo.toml` constraint will catch it; this test
    /// documents the *intent* of the port abstraction.
    #[test]
    fn event_sink_is_trait_erased_not_concrete_store() {
        use std::sync::{Arc, Mutex};

        // Any type implementing Fn(&Event) can serve as the event sink.
        // This struct stands in for what braid-observe::SessionWriter does,
        // but braid-core has zero knowledge of that concrete type.
        struct FakeSink {
            events: Vec<EventKind>,
        }
        let sink: Arc<Mutex<FakeSink>> = Arc::new(Mutex::new(FakeSink { events: vec![] }));
        let sink_ref = Arc::clone(&sink);

        // Engine::with_event_callback accepts Box<dyn Fn(&Event)> — a port, not a
        // concrete type. This is the key invariant.
        let engine = Engine::new(crate::tools::StaticTool::new("echo", "out"), TestProvider)
            .with_event_callback(move |e: &Event| {
                sink_ref.lock().unwrap().events.push(e.kind.clone());
            });

        engine
            .run(
                RunInput {
                    session_id: SessionId("port-boundary".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "boundary check".into(),
                        }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let events = &sink.lock().unwrap().events;
        assert!(
            events.contains(&EventKind::SessionStarted),
            "SessionStarted must flow through the port"
        );
        assert!(
            events.contains(&EventKind::SessionCompleted),
            "SessionCompleted must flow through the port"
        );
        // The engine emits events; the concrete sink (SessionWriter) is wired
        // only at the CLI composition root — never inside braid-core.
    }

    #[test]
    fn event_callback_fires_for_each_event() {
        use std::sync::{Arc, Mutex};

        let fired: Arc<Mutex<Vec<EventKind>>> = Arc::new(Mutex::new(vec![]));
        let fired_clone = Arc::clone(&fired);

        let engine = Engine::new(crate::tools::StaticTool::new("echo", "out"), TestProvider)
            .with_event_callback(move |e: &Event| {
                fired_clone.lock().unwrap().push(e.kind.clone());
            });

        engine
            .run(
                RunInput {
                    session_id: SessionId("cb-1".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text { text: "hi".into() }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let kinds = fired.lock().unwrap();
        assert!(kinds.contains(&EventKind::SessionStarted));
        assert!(kinds.contains(&EventKind::SessionCompleted));
    }

    /// Provider that makes N tool calls (one per turn) before returning a final text response.
    struct MultiToolProvider {
        tool_calls: u32,
        call_count: std::cell::Cell<u32>,
    }

    impl MultiToolProvider {
        fn new(tool_calls: u32) -> Self {
            Self {
                tool_calls,
                call_count: std::cell::Cell::new(0),
            }
        }
    }

    impl Provider for MultiToolProvider {
        fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);

            if count < self.tool_calls {
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::ToolUse {
                            id: format!("call_{count}"),
                            name: "echo".into(),
                            input: serde_json::json!({"n": count}),
                        }],
                    },
                    token_count: None,
                })
            } else {
                Ok(ProviderResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentPart::Text {
                            text: "all done".into(),
                        }],
                    },
                    token_count: None,
                })
            }
        }
    }

    #[test]
    fn event_callback_emits_in_deterministic_causal_order() {
        use std::sync::{Arc, Mutex};

        // Collect events from callback into a separate Vec so we can compare
        // the callback-delivery order against RunOutput.events.
        let callback_events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(vec![]));
        let callback_events_clone = Arc::clone(&callback_events);

        // Use a provider that makes 2 tool calls before finishing (3 provider turns total).
        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "echoed"),
            MultiToolProvider::new(2),
        )
        .with_event_callback(move |e: &Event| {
            callback_events_clone.lock().unwrap().push(e.clone());
        });

        let output = engine
            .run(
                RunInput {
                    session_id: SessionId("ordering-test".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text { text: "go".into() }],
                    }],
                    max_turns: None,
                },
                &SimpleLoopPlanner,
            )
            .unwrap();

        let cb = callback_events.lock().unwrap();

        // ── Invariant 1: callback received exactly as many events as RunOutput ──
        assert_eq!(
            cb.len(),
            output.events.len(),
            "callback event count must match RunOutput.events"
        );

        // ── Invariant 2: callback order matches RunOutput.events order exactly ──
        for (i, (cb_evt, out_evt)) in cb.iter().zip(output.events.iter()).enumerate() {
            assert_eq!(
                std::mem::discriminant(&cb_evt.kind),
                std::mem::discriminant(&out_evt.kind),
                "event at index {i} differs between callback and RunOutput"
            );
        }

        // ── Invariant 3: strict causal ordering ──
        // Expected sequence for 2 tool calls:
        //   [0] SessionStarted
        //   [1] ProviderResponded  (turn 1 → tool call 0)
        //   [2] ToolCalled
        //   [3] ToolCompleted
        //   [4] ProviderResponded  (turn 2 → tool call 1)
        //   [5] ToolCalled
        //   [6] ToolCompleted
        //   [7] ProviderResponded  (turn 3 → final)
        //   [8] SessionCompleted
        let kinds: Vec<&EventKind> = output.events.iter().map(|e| &e.kind).collect();

        assert!(
            matches!(kinds[0], EventKind::SessionStarted),
            "first event must be SessionStarted, got {:?}",
            kinds[0]
        );
        assert!(
            matches!(kinds.last().unwrap(), EventKind::SessionCompleted),
            "last event must be SessionCompleted, got {:?}",
            kinds.last().unwrap()
        );

        // No duplicate SessionStarted or SessionCompleted
        let started_count = kinds
            .iter()
            .filter(|k| matches!(k, EventKind::SessionStarted))
            .count();
        let completed_count = kinds
            .iter()
            .filter(|k| matches!(k, EventKind::SessionCompleted))
            .count();
        assert_eq!(started_count, 1, "exactly one SessionStarted");
        assert_eq!(completed_count, 1, "exactly one SessionCompleted");

        // Every ToolCalled must be immediately followed by ToolCompleted (same tool name)
        let inner = &kinds[1..kinds.len() - 1]; // strip SessionStarted / SessionCompleted
        let mut i = 0;
        while i < inner.len() {
            match inner[i] {
                EventKind::ToolCalled { tool_name } => {
                    assert!(
                        i + 1 < inner.len(),
                        "ToolCalled at index {i} has no following event"
                    );
                    match inner[i + 1] {
                        EventKind::ToolCompleted {
                            tool_name: completed_name,
                        } => {
                            assert_eq!(
                                tool_name, completed_name,
                                "ToolCalled/ToolCompleted tool name mismatch at index {i}"
                            );
                        }
                        other => panic!(
                            "expected ToolCompleted after ToolCalled at index {i}, got {other:?}"
                        ),
                    }
                    i += 2; // consume both
                }
                _ => i += 1,
            }
        }

        // Total event count: 1 (start) + 3*ProviderResponded + 2*(ToolCalled+ToolCompleted) + 1 (end)
        // = 1 + 3 + 4 + 1 = 9
        assert_eq!(
            output.events.len(),
            9,
            "expected 9 events for 2-tool session"
        );
    }

    #[test]
    fn errors_at_max_turns_with_no_response() {
        // Provider that always fails — max_turns=0 means we never call it
        struct NeverProvider;
        impl Provider for NeverProvider {
            fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
                panic!("should not be called");
            }
        }

        let engine = Engine::new(crate::tools::StaticTool::new("echo", "out"), NeverProvider);
        let err = engine
            .run(
                RunInput {
                    session_id: SessionId("session-4".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text { text: "go".into() }],
                    }],
                    max_turns: Some(0),
                },
                &SimpleLoopPlanner,
            )
            .unwrap_err();

        assert!(err.to_string().contains("max turns"));
    }
}
