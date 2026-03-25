use anyhow::Result;
use braid_model::{
    ContentPart, Event, EventKind, Message, ProviderRequest, Role, SessionId, ToolCall, ToolResult,
};
use braid_ports::{EventSink, Provider, Redactor, ToolExecutor};

use crate::planner::{Action, Planner, SessionState};

const DEFAULT_MAX_TURNS: u32 = 10;

#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub max_turns: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub provider_response: braid_model::ProviderResponse,
}

pub struct Engine<P, T, S, R> {
    provider: P,
    tool_executor: T,
    event_sink: S,
    redactor: R,
}

impl<P, T, S, R> Engine<P, T, S, R>
where
    P: Provider,
    T: ToolExecutor,
    S: EventSink,
    R: Redactor,
{
    pub fn new(provider: P, tool_executor: T, event_sink: S, redactor: R) -> Self {
        Self {
            provider,
            tool_executor,
            event_sink,
            redactor,
        }
    }

    pub fn run(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput> {
        let result = self.run_inner(input, planner);
        let _ = self.event_sink.flush(); // best-effort on error path
        result
    }

    fn run_inner(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput> {
        let max_turns = input.max_turns.unwrap_or(DEFAULT_MAX_TURNS);
        let mut state = SessionState {
            messages: input.messages,
            pending_tool_calls: vec![],
            last_provider_response: None,
            turn_count: 0,
            max_turns,
        };

        self.event_sink.record(&Event {
            session_id: input.session_id.clone(),
            kind: EventKind::SessionStarted,
        })?;

        loop {
            let action = planner.next_action(&state)?;

            match action {
                Action::CallProvider { messages } => {
                    let messages: Vec<Message> = messages
                        .iter()
                        .map(|m| self.redactor.redact_message(m))
                        .collect();
                    let response = self.provider.complete(ProviderRequest {
                        messages,
                        tools: vec![],
                    })?;

                    state.messages.push(response.message.clone());
                    state.pending_tool_calls = extract_tool_calls(&response.message);
                    state.last_provider_response = Some(response);
                    state.turn_count += 1;

                    self.event_sink.record(&Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ProviderResponded,
                    })?;
                }
                Action::ExecuteTool { call } => {
                    self.event_sink.record(&Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCalled {
                            tool_name: call.name.clone(),
                        },
                    })?;

                    let result = self.tool_executor.execute(call.clone())?;
                    let result_msg = tool_result_to_message(&call, &result);
                    let redacted_msg = self.redactor.redact_message(&result_msg);
                    state.messages.push(redacted_msg);
                    state.pending_tool_calls.retain(|c| c.id != call.id);

                    self.event_sink.record(&Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCompleted {
                            tool_name: call.name.clone(),
                        },
                    })?;
                }
                Action::Finish { response } => {
                    self.event_sink.record(&Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::SessionCompleted,
                    })?;
                    self.event_sink.flush()?;

                    return Ok(RunOutput {
                        provider_response: response,
                    });
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::SimpleLoopPlanner;
    use braid_model::{ContentPart, EventKind, ProviderResponse, Role, SessionId};
    use braid_ports::{EventSink, Redactor};
    use std::sync::{Arc, Mutex};

    // ── inline test doubles ──────────────────────────────────────────────────

    #[derive(Clone)]
    struct VecSink(Arc<Mutex<Vec<Event>>>);

    impl VecSink {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(vec![])))
        }
        fn events(&self) -> Vec<Event> {
            self.0.lock().unwrap().clone()
        }
    }

    impl EventSink for VecSink {
        fn record(&self, event: &Event) -> anyhow::Result<()> {
            self.0.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    struct Passthrough;
    impl Redactor for Passthrough {
        fn redact_message(&self, msg: &Message) -> Message {
            msg.clone()
        }
    }

    struct TestProvider;
    impl Provider for TestProvider {
        fn complete(&self, request: ProviderRequest) -> anyhow::Result<ProviderResponse> {
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
        fn complete(&self, _request: ProviderRequest) -> anyhow::Result<ProviderResponse> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);
            if count == 0 {
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

    // ── tests ────────────────────────────────────────────────────────────────

    #[test]
    fn runs_a_minimal_session() {
        let sink = VecSink::new();
        let engine = Engine::new(
            TestProvider,
            crate::tools::StaticTool::new("echo", "tool output"),
            sink.clone(),
            Passthrough,
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

        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0].kind, EventKind::SessionStarted));
        assert!(matches!(events[1].kind, EventKind::ProviderResponded));
        assert!(matches!(events[2].kind, EventKind::SessionCompleted));
    }

    #[test]
    fn runs_tool_call_loop() {
        let sink = VecSink::new();
        let engine = Engine::new(
            ToolCallingProvider::new(),
            crate::tools::StaticTool::new("echo", "echoed output"),
            sink.clone(),
            Passthrough,
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

        let events = sink.events();
        assert_eq!(events.len(), 6);
        assert!(matches!(events[2].kind, EventKind::ToolCalled { .. }));
        assert!(matches!(events[3].kind, EventKind::ToolCompleted { .. }));
    }

    #[test]
    fn respects_max_turns() {
        struct InfiniteToolProvider;
        impl Provider for InfiniteToolProvider {
            fn complete(&self, _request: ProviderRequest) -> anyhow::Result<ProviderResponse> {
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

        let sink = VecSink::new();
        let engine = Engine::new(
            InfiniteToolProvider,
            crate::tools::StaticTool::new("echo", "out"),
            sink.clone(),
            Passthrough,
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

        let events = sink.events();
        assert!(matches!(
            events.last().unwrap().kind,
            EventKind::SessionCompleted
        ));
        let _ = output;
    }

    #[test]
    fn redactor_transforms_messages_before_provider() {
        struct EchoProvider;
        impl Provider for EchoProvider {
            fn complete(&self, request: ProviderRequest) -> anyhow::Result<ProviderResponse> {
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

        struct ReplaceRedactor;
        impl Redactor for ReplaceRedactor {
            fn redact_message(&self, msg: &Message) -> Message {
                Message {
                    role: msg.role.clone(),
                    content: msg
                        .content
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => ContentPart::Text {
                                text: text.replace("secret", "[R]"),
                            },
                            other => other.clone(),
                        })
                        .collect(),
                }
            }
        }

        let engine = Engine::new(
            EchoProvider,
            crate::tools::StaticTool::new("echo", "out"),
            VecSink::new(),
            ReplaceRedactor,
        );

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
        assert!(response_text.contains("[R]"), "redactor should have fired");
        assert!(
            !response_text.contains("secret"),
            "raw secret should not reach provider"
        );
    }

    #[test]
    fn errors_at_max_turns_with_no_response() {
        struct NeverProvider;
        impl Provider for NeverProvider {
            fn complete(&self, _request: ProviderRequest) -> anyhow::Result<ProviderResponse> {
                panic!("should not be called");
            }
        }

        let engine = Engine::new(
            NeverProvider,
            crate::tools::StaticTool::new("echo", "out"),
            VecSink::new(),
            Passthrough,
        );
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
