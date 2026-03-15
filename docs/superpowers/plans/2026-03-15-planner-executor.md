# Planner/Executor Separation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate planning decisions from execution in braid-core's engine, enabling a loop that handles tool calls across multiple turns.

**Architecture:** Add a `Planner` trait that decides the next `Action` (CallProvider, ExecuteTool, Finish) based on `SessionState`. Implement `SimpleLoopPlanner` as the default. Rewrite `Engine::run` as a planner-driven loop. Add `id` to `ToolCall` for correlation.

**Tech Stack:** Rust 1.88, edition 2024, no new crate dependencies

**Spec:** `docs/superpowers/specs/2026-03-15-planner-executor-design.md`

---

## Chunk 1: Model Changes

### Task 1: Rename SessionState → SessionPhase in braid-model

**Files:**
- Modify: `crates/braid-model/src/session.rs`
- Modify: `crates/braid-model/src/lib.rs`
- Modify: `crates/braid-model/tests/serde_roundtrip.rs`

- [ ] **Step 1: Rename enum in session.rs**

Replace `SessionState` with `SessionPhase` in `crates/braid-model/src/session.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionPhase {
    Planned,
    Running,
    WaitingForTool,
    Completed,
}
```

- [ ] **Step 2: Update lib.rs export**

In `crates/braid-model/src/lib.rs`, change line 12:

```rust
pub use session::{SessionId, SessionPhase};
```

- [ ] **Step 3: Update serde roundtrip test**

In `crates/braid-model/tests/serde_roundtrip.rs`, rename the test and type references:

```rust
#[test]
fn session_phase_roundtrip() {
    roundtrip(&SessionPhase::Planned);
    roundtrip(&SessionPhase::Running);
    roundtrip(&SessionPhase::WaitingForTool);
    roundtrip(&SessionPhase::Completed);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: All tests pass. No other code references `SessionState` (it was unused outside model).

- [ ] **Step 5: Commit**

```bash
git add crates/braid-model/src/session.rs crates/braid-model/src/lib.rs crates/braid-model/tests/serde_roundtrip.rs
git commit -m "refactor: rename SessionState to SessionPhase in braid-model"
```

### Task 2: Add ToolCall.id field and SessionCompleted event

**Files:**
- Modify: `crates/braid-model/src/tool.rs`
- Modify: `crates/braid-model/src/event.rs`
- Modify: `crates/braid-model/tests/serde_roundtrip.rs`
- Modify: `crates/braid-core/src/tools.rs`
- Modify: `crates/braid-core/src/registry.rs`
- Modify: `crates/braid-cli/src/main.rs` (won't need changes — doesn't construct ToolCall directly anymore)

- [ ] **Step 1: Add id field to ToolCall**

Replace `crates/braid-model/src/tool.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    pub name: String,
    pub output: String,
}
```

- [ ] **Step 2: Add SessionCompleted to EventKind**

In `crates/braid-model/src/event.rs`, add the variant:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    SessionStarted,
    ToolCalled { tool_name: String },
    ToolCompleted { tool_name: String },
    ProviderResponded,
    SessionCompleted,
}
```

- [ ] **Step 3: Update serde roundtrip tests**

In `crates/braid-model/tests/serde_roundtrip.rs`, update `tool_call_roundtrip` and add `SessionCompleted` test:

```rust
#[test]
fn tool_call_roundtrip() {
    roundtrip(&ToolCall {
        id: "call_1".into(),
        name: "echo".into(),
        input: "hello".into(),
    });
}
```

Add to the `event_roundtrip` test:

```rust
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::SessionCompleted,
    });
```

(Insert this before the final `roundtrip` that consumes `sid` — change the last `roundtrip` call to clone `sid` too, or insert this one before it. Simplest: add it right before the ToolCompleted one and change that final call to also clone.)

Update the full event test to:

```rust
#[test]
fn event_roundtrip() {
    let sid = SessionId("s1".into());
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::SessionStarted,
    });
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::ProviderResponded,
    });
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::ToolCalled {
            tool_name: "echo".into(),
        },
    });
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::ToolCompleted {
            tool_name: "echo".into(),
        },
    });
    roundtrip(&Event {
        session_id: sid,
        kind: EventKind::SessionCompleted,
    });
}
```

- [ ] **Step 4: Fix compilation — update StaticTool and ToolRegistry**

In `crates/braid-core/src/tools.rs`, `StaticTool::execute` now receives a `ToolCall` with an `id` field. No logic change needed — it just ignores `id`:

```rust
impl ToolExecutor for StaticTool {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        Ok(ToolResult {
            name: call.name.if_empty_then(self.name.clone()),
            output: self.output.clone(),
        })
    }
}
```

This doesn't change, but the **tests** that construct `ToolCall` need updating.

In `crates/braid-core/src/registry.rs`, update the test `ToolCall` constructions:

```rust
    #[test]
    fn execute_dispatches_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register("echo", Box::new(StaticTool::new("echo", "echoed")));
        let result = registry.execute(ToolCall {
            id: "call_1".into(),
            name: "echo".into(),
            input: "hello".into(),
        }).unwrap();
        assert_eq!(result.name, "echo");
        assert_eq!(result.output, "echoed");
    }

    #[test]
    fn execute_unknown_tool_errors() {
        let registry = ToolRegistry::new();
        let err = registry.execute(ToolCall {
            id: "call_1".into(),
            name: "missing".into(),
            input: "".into(),
        }).unwrap_err();
        assert!(err.to_string().contains("tool not found: missing"));
    }
```

In `crates/braid-core/src/engine.rs`, update the test `ToolCall` construction (it's no longer used in RunInput but may be referenced). Actually, the current engine test doesn't construct ToolCall — it was removed in the earlier refactor. No change needed.

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-model/src/tool.rs crates/braid-model/src/event.rs crates/braid-model/tests/serde_roundtrip.rs crates/braid-core/src/registry.rs
git commit -m "feat: add ToolCall.id field and SessionCompleted event kind"
```

---

## Chunk 2: Planner Infrastructure

### Task 3: Add Action, Planner trait, SessionState, and SimpleLoopPlanner

**Files:**
- Create: `crates/braid-core/src/planner.rs`
- Modify: `crates/braid-core/src/lib.rs`

- [ ] **Step 1: Create planner.rs with all types, trait, and SimpleLoopPlanner**

Create `crates/braid-core/src/planner.rs`:

```rust
use anyhow::{bail, Result};
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
/// 1. If turn_count >= max_turns, finish with last response (or error).
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
            return Ok(Action::ExecuteTool {
                call: call.clone(),
            });
        }

        // 3. Provider responded with no tool calls — done
        if let Some(response) = &state.last_provider_response {
            return Ok(Action::Finish {
                response: response.clone(),
            });
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
    use braid_model::{ContentPart, Message, Role, TokenCount};

    fn make_text_response(text: &str) -> ProviderResponse {
        ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text {
                    text: text.into(),
                }],
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
```

- [ ] **Step 2: Export from lib.rs**

Update `crates/braid-core/src/lib.rs`:

```rust
pub mod engine;
pub mod planner;
pub mod registry;
pub mod tools;

pub use engine::{Engine, RunInput, RunOutput};
pub use planner::{Action, Planner, SessionState, SimpleLoopPlanner};
pub use registry::ToolRegistry;
pub use tools::{StaticTool, ToolExecutor};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p braid-core`
Expected: All tests pass (existing engine test + 4 registry tests + 5 new planner tests).

- [ ] **Step 4: Commit**

```bash
git add crates/braid-core/src/planner.rs crates/braid-core/src/lib.rs
git commit -m "feat: add Planner trait, Action enum, SessionState, and SimpleLoopPlanner"
```

---

## Chunk 3: Engine Loop Rewrite

### Task 4: Rewrite Engine::run as a planner-driven loop

**Files:**
- Modify: `crates/braid-core/src/engine.rs`

- [ ] **Step 1: Add max_turns to RunInput**

In `crates/braid-core/src/engine.rs`, update `RunInput`:

```rust
#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub max_turns: Option<u32>,
}
```

- [ ] **Step 2: Rewrite Engine::run**

Replace the `Engine::run` method and add helper functions. The complete new `engine.rs`:

```rust
use anyhow::Result;
use braid_model::{
    ContentPart, Event, EventKind, Message, ProviderRequest, ProviderResponse,
    Role, SessionId, ToolCall, ToolResult,
};

use crate::planner::{Action, Planner, SessionState};
use crate::tools::ToolExecutor;

const DEFAULT_MAX_TURNS: u32 = 10;

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

        events.push(Event {
            session_id: input.session_id.clone(),
            kind: EventKind::SessionStarted,
        });

        loop {
            let action = planner.next_action(&state)?;

            match action {
                Action::CallProvider { messages } => {
                    let response = self.provider.complete(ProviderRequest { messages })?;

                    // Add assistant message to conversation
                    state.messages.push(response.message.clone());

                    // Extract tool calls from response
                    state.pending_tool_calls = extract_tool_calls(&response.message);
                    state.last_provider_response = Some(response);
                    state.turn_count += 1;

                    events.push(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ProviderResponded,
                    });
                }
                Action::ExecuteTool { call } => {
                    events.push(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCalled {
                            tool_name: call.name.clone(),
                        },
                    });

                    let result = self.tool_executor.execute(call.clone())?;

                    // Add tool result as message
                    state.messages.push(tool_result_to_message(&call, &result));

                    // Remove executed call from pending
                    state
                        .pending_tool_calls
                        .retain(|c| c.id != call.id);

                    events.push(Event {
                        session_id: input.session_id.clone(),
                        kind: EventKind::ToolCompleted {
                            tool_name: call.name.clone(),
                        },
                    });
                }
                Action::Finish { response } => {
                    events.push(Event {
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
    use braid_model::ContentPart;
    use crate::planner::SimpleLoopPlanner;

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
        assert!(matches!(output.events[1].kind, EventKind::ProviderResponded));
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
        assert!(matches!(output.events[2].kind, EventKind::ToolCalled { .. }));
        assert!(matches!(output.events[3].kind, EventKind::ToolCompleted { .. }));
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
                        content: vec![ContentPart::Text {
                            text: "go".into(),
                        }],
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
    fn errors_at_max_turns_with_no_response() {
        // Provider that always fails — max_turns=0 means we never call it
        struct NeverProvider;
        impl Provider for NeverProvider {
            fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
                panic!("should not be called");
            }
        }

        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "out"),
            NeverProvider,
        );
        let err = engine
            .run(
                RunInput {
                    session_id: SessionId("session-4".into()),
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "go".into(),
                        }],
                    }],
                    max_turns: Some(0),
                },
                &SimpleLoopPlanner,
            )
            .unwrap_err();

        assert!(err.to_string().contains("max turns"));
    }
}
```

- [ ] **Step 3: Add serde_json dev-dependency to braid-core**

The `ToolCallingProvider` test uses `serde_json::json!()`. Add to `crates/braid-core/Cargo.toml`:

```toml
[dev-dependencies]
serde_json.workspace = true
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p braid-core`
Expected: All tests pass (4 engine tests + 4 registry tests + 5 planner tests).

- [ ] **Step 5: Commit**

```bash
git add crates/braid-core/src/engine.rs crates/braid-core/Cargo.toml
git commit -m "feat: rewrite Engine::run as planner-driven loop with tool call support"
```

### Task 5: Update CLI to pass planner

**Files:**
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Update CLI to pass planner and max_turns**

In `crates/braid-cli/src/main.rs`, update the `cmd_run` function. Add import for `SimpleLoopPlanner` and update the `engine.run()` call:

Change the import line:
```rust
use braid_core::{Engine, RunInput, SimpleLoopPlanner, ToolRegistry};
```

Update `cmd_run`:
```rust
fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let engine = Engine::new(ToolRegistry::new(), provider);
    let output = engine.run(
        RunInput {
            session_id: SessionId("session".into()),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: prompt }],
            }],
            max_turns: None,
        },
        &SimpleLoopPlanner,
    )?;

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{}", response_text);
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}
```

- [ ] **Step 2: Run full workspace tests and CLI**

Run: `cargo test --workspace && cargo run -p braid-cli -- run --provider mock "hello"`
Expected: All tests pass, CLI prints mock response.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-cli/src/main.rs
git commit -m "feat: update CLI to use planner-driven engine loop"
```
