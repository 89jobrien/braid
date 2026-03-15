use anyhow::Result;
use braid_model::{
    Event, EventKind, Message, ProviderRequest, ProviderResponse, SessionId,
};

use crate::tools::ToolExecutor;

#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub provider_response: ProviderResponse,
    pub events: Vec<Event>,
}

#[derive(Debug)]
pub struct Engine<T, P> {
    tool_executor: T,
    provider: P,
}

impl<T, P> Engine<T, P> {
    pub fn new(tool_executor: T, provider: P) -> Self {
        Self {
            tool_executor,
            provider,
        }
    }
}

pub trait Provider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

impl<T, P> Engine<T, P>
where
    T: ToolExecutor,
    P: Provider,
{
    pub fn run(&self, input: RunInput) -> Result<RunOutput> {
        let provider_response = self.provider.complete(ProviderRequest {
            messages: input.messages,
        })?;
        let events = vec![
            Event {
                session_id: input.session_id.clone(),
                kind: EventKind::SessionStarted,
            },
            Event {
                session_id: input.session_id,
                kind: EventKind::ProviderResponded,
            },
        ];

        Ok(RunOutput {
            provider_response,
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{ContentPart, Role};

    struct TestProvider;

    impl Provider for TestProvider {
        fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
            let first_text = request.messages.iter()
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

    #[test]
    fn runs_a_minimal_session() {
        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "tool output"),
            TestProvider,
        );
        let output = engine
            .run(RunInput {
                session_id: SessionId("session-1".into()),
                messages: vec![Message {
                    role: Role::User,
                    content: vec![ContentPart::Text {
                        text: "hello".into(),
                    }],
                }],
            })
            .unwrap();

        let response_text = match &output.provider_response.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(response_text, "provider saw: hello");
        assert_eq!(output.events.len(), 2);
    }
}
