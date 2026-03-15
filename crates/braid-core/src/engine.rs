use anyhow::Result;
use braid_model::{
    Event, EventKind, ProviderRequest, ProviderResponse, SessionId, ToolCall, ToolResult,
};

use crate::tools::ToolExecutor;

#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub prompt: String,
    pub tool: ToolCall,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub provider_response: ProviderResponse,
    pub tool_result: ToolResult,
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
            prompt: input.prompt,
        })?;
        let tool_result = self.tool_executor.execute(input.tool.clone())?;
        let events = vec![
            Event {
                session_id: input.session_id.clone(),
                kind: EventKind::SessionStarted,
            },
            Event {
                session_id: input.session_id.clone(),
                kind: EventKind::ProviderResponded,
            },
            Event {
                session_id: input.session_id.clone(),
                kind: EventKind::ToolCalled {
                    tool_name: input.tool.name.clone(),
                },
            },
            Event {
                session_id: input.session_id,
                kind: EventKind::ToolCompleted {
                    tool_name: tool_result.name.clone(),
                },
            },
        ];

        Ok(RunOutput {
            provider_response,
            tool_result,
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::StaticTool;

    struct TestProvider;

    impl Provider for TestProvider {
        fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
            Ok(ProviderResponse {
                message: format!("provider saw: {}", request.prompt),
            })
        }
    }

    #[test]
    fn runs_a_minimal_session() {
        let engine = Engine::new(StaticTool::new("echo", "tool output"), TestProvider);
        let output = engine
            .run(RunInput {
                session_id: SessionId("session-1".into()),
                prompt: "hello".into(),
                tool: ToolCall {
                    name: "echo".into(),
                    input: "run".into(),
                },
            })
            .unwrap();

        assert_eq!(output.provider_response.message, "provider saw: hello");
        assert_eq!(output.tool_result.output, "tool output");
        assert_eq!(output.events.len(), 4);
    }
}
