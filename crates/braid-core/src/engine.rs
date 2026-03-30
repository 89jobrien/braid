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
