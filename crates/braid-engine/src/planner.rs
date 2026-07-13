use anyhow::{Result, bail};
use braid_model::{Message, ProviderResponse, ToolCall};

/// The current state of an in-progress session, inspected by the planner.
#[derive(Debug, Clone)]
pub struct SessionState {
    pub messages: Vec<Message>,
    pub pending_tool_calls: Vec<ToolCall>,
    pub last_provider_response: Option<ProviderResponse>,
    pub turn_count: u32,
    pub max_turns: u32,
}

/// An action the engine should execute next.
#[derive(Debug, Clone)]
pub enum Action {
    CallProvider { messages: Vec<Message> },
    ExecuteTool { call: ToolCall },
    Finish { response: ProviderResponse },
}

/// Decides what the engine should do next based on session state.
pub trait Planner {
    fn next_action(&self, state: &SessionState) -> Result<Action>;
}

/// Default planner: standard tool-call loop.
///
/// 1. If `turn_count` >= `max_turns`, finish with last response (or error).
/// 2. If pending tool calls, execute the first one.
/// 3. If last response exists and no pending tools, finish.
/// 4. Otherwise, call the provider.
#[derive(Debug, Clone, Default)]
pub struct SimpleLoopPlanner;

impl Planner for SimpleLoopPlanner {
    fn next_action(&self, state: &SessionState) -> Result<Action> {
        // 1. Turn limit reached
        if state.turn_count >= state.max_turns {
            return match &state.last_provider_response {
                Some(response) => Ok(Action::Finish {
                    response: response.clone(),
                }),
                None => bail!(
                    "reached max turns ({}) without a provider response",
                    state.max_turns
                ),
            };
        }

        // 2. Pending tool calls — execute next one
        if let Some(call) = state.pending_tool_calls.first() {
            return Ok(Action::ExecuteTool { call: call.clone() });
        }

        // 3. Provider responded — finish only if it was a pure text response
        //    (no tool calls). If the last response contained tool calls and all
        //    have been executed, we fall through to call the provider again.
        if let Some(response) = &state.last_provider_response {
            let had_tool_calls = response
                .message
                .content
                .iter()
                .any(|part| matches!(part, braid_model::ContentPart::ToolUse { .. }));
            if !had_tool_calls {
                return Ok(Action::Finish {
                    response: response.clone(),
                });
            }
        }

        // 4. Need to call provider
        Ok(Action::CallProvider {
            messages: state.messages.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{ContentPart, Message, Role};

    fn make_text_response(text: &str) -> ProviderResponse {
        ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text { text: text.into() }],
            },
            token_count: None,
        }
    }

    #[test]
    fn calls_provider_when_no_state() {
        let planner = SimpleLoopPlanner;
        let state = SessionState {
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: "hello".into(),
                }],
            }],
            pending_tool_calls: vec![],
            last_provider_response: None,
            turn_count: 0,
            max_turns: 10,
        };
        let action = planner.next_action(&state).unwrap();
        assert!(matches!(action, Action::CallProvider { .. }));
    }

    #[test]
    fn executes_tool_when_pending() {
        let planner = SimpleLoopPlanner;
        let state = SessionState {
            messages: vec![],
            pending_tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "echo".into(),
                input: "hi".into(),
            }],
            last_provider_response: Some(make_text_response("use echo")),
            turn_count: 1,
            max_turns: 10,
        };
        let action = planner.next_action(&state).unwrap();
        assert!(matches!(action, Action::ExecuteTool { .. }));
    }

    #[test]
    fn finishes_when_no_pending_tools() {
        let planner = SimpleLoopPlanner;
        let state = SessionState {
            messages: vec![],
            pending_tool_calls: vec![],
            last_provider_response: Some(make_text_response("done")),
            turn_count: 1,
            max_turns: 10,
        };
        let action = planner.next_action(&state).unwrap();
        assert!(matches!(action, Action::Finish { .. }));
    }

    #[test]
    fn finishes_at_turn_limit() {
        let planner = SimpleLoopPlanner;
        let state = SessionState {
            messages: vec![],
            pending_tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "echo".into(),
                input: "hi".into(),
            }],
            last_provider_response: Some(make_text_response("still going")),
            turn_count: 10,
            max_turns: 10,
        };
        let action = planner.next_action(&state).unwrap();
        assert!(matches!(action, Action::Finish { .. }));
    }

    #[test]
    fn errors_at_turn_limit_with_no_response() {
        let planner = SimpleLoopPlanner;
        let state = SessionState {
            messages: vec![],
            pending_tool_calls: vec![],
            last_provider_response: None,
            turn_count: 10,
            max_turns: 10,
        };
        let err = planner.next_action(&state).unwrap_err();
        assert!(err.to_string().contains("max turns"));
    }
}
