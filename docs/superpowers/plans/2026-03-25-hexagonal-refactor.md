# Hexagonal Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `braid-ports` as the hexagonal inner ring, promote `EventSink` and `SessionStorage` to ports, and wire `Engine<P,T,S,R>` through all adapter crates.

**Architecture:** A new `braid-ports` crate (deps: `braid-model` only) holds all six port traits. Every adapter crate depends on `braid-ports` rather than each other. `braid-core`'s `Engine` gains `EventSink` and `Redactor` generic params; `RunOutput` drops its `Vec<Event>`.

**Tech Stack:** Rust 2024 edition, `cargo nextest`, workspace Cargo.toml, `just` for task commands.

---

## File Map

**Create:**
- `crates/braid-ports/Cargo.toml`
- `crates/braid-ports/src/lib.rs`
- `crates/braid-providers/src/mock.rs`
- `crates/braid-observe/src/memory.rs`

**Modify:**
- `Cargo.toml` — add `braid-ports` to workspace members
- `crates/braid-core/Cargo.toml` — add `braid-ports` dep
- `crates/braid-core/src/lib.rs` — re-export port traits from braid-ports
- `crates/braid-core/src/engine.rs` — `Engine<P,T,S,R>`, push events to sink, drop closure redactor
- `crates/braid-core/src/tools.rs` — drop `ToolExecutor` def, keep `StaticTool`
- `crates/braid-core/src/registry.rs` — change `ToolExecutor` import source
- `crates/braid-providers/Cargo.toml` — swap `braid-core` for `braid-ports`
- `crates/braid-providers/src/lib.rs` — add `MockProvider` export behind feature
- `crates/braid-providers/src/openai.rs` — change `Provider` import to `braid_ports`
- `crates/braid-hooks/Cargo.toml` — swap `braid-core` for `braid-ports`
- `crates/braid-hooks/src/lib.rs` — update re-exports
- `crates/braid-hooks/src/contract.rs` — delete content; replace with re-exports from braid-ports
- `crates/braid-hooks/src/executor.rs` — change `ToolExecutor` import to `braid_ports`
- `crates/braid-redact/Cargo.toml` — add `braid-ports` dep
- `crates/braid-redact/src/lib.rs` — no change needed
- `crates/braid-redact/src/pipeline.rs` — add `impl Redactor for RedactionPipeline`
- `crates/braid-observe/Cargo.toml` — add `braid-ports` dep
- `crates/braid-observe/src/lib.rs` — add `InMemorySessionStorage` export
- `crates/braid-observe/src/store.rs` — add `Mutex<Vec<Event>>` buffer, `EventSink` + `SessionStorage` impls
- `crates/braid-cli/src/main.rs` — rewrite `cmd_run`, update imports

---

## Task 1: Create `braid-ports`

**Files:**
- Create: `crates/braid-ports/Cargo.toml`
- Create: `crates/braid-ports/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add braid-ports to workspace**

Edit `Cargo.toml` — add `"crates/braid-ports"` to the `members` array:

```toml
[workspace]
members = [
  "crates/braid-model",
  "crates/braid-ports",
  "crates/braid-core",
  "crates/braid-providers",
  "crates/braid-cli",
  "crates/braid-redact",
  "crates/braid-hooks",
  "crates/braid-mcp",
  "crates/braid-observe",
]
```

- [ ] **Step 2: Create Cargo.toml for braid-ports**

```toml
# crates/braid-ports/Cargo.toml
[package]
name = "braid-ports"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
```

- [ ] **Step 3: Write braid-ports/src/lib.rs with all port trait definitions**

```rust
// crates/braid-ports/src/lib.rs
use anyhow::Result;
use braid_model::{Event, Message, ProviderRequest, ProviderResponse, SessionId, ToolCall, ToolResult};
use std::sync::Arc;

// ── Provider ────────────────────────────────────────────────────────────────

pub trait Provider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

impl<T: Provider + ?Sized> Provider for Box<T> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}

impl<T: Provider + ?Sized> Provider for Arc<T> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}

// ── ToolExecutor ─────────────────────────────────────────────────────────────

pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}

impl<T: ToolExecutor + ?Sized> ToolExecutor for Box<T> {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        (**self).execute(call)
    }
}

impl<T: ToolExecutor + ?Sized> ToolExecutor for Arc<T> {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        (**self).execute(call)
    }
}

// ── Redactor ─────────────────────────────────────────────────────────────────

pub trait Redactor {
    fn redact_message(&self, msg: &Message) -> Message;
}

impl<T: Redactor + ?Sized> Redactor for Arc<T> {
    fn redact_message(&self, msg: &Message) -> Message {
        (**self).redact_message(msg)
    }
}

// ── EventSink ────────────────────────────────────────────────────────────────

pub trait EventSink {
    fn record(&self, event: &Event) -> Result<()>;
    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

impl<T: EventSink + ?Sized> EventSink for Arc<T> {
    fn record(&self, event: &Event) -> Result<()> {
        (**self).record(event)
    }
    fn flush(&self) -> Result<()> {
        (**self).flush()
    }
}

// ── SessionStorage ───────────────────────────────────────────────────────────

pub trait SessionStorage {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()>;
    fn load(&self, id: &SessionId) -> Result<Vec<Event>>;
    fn list(&self) -> Result<Vec<SessionId>>;
    fn prune(&self, keep: usize) -> Result<usize>;
}

impl<T: SessionStorage + ?Sized> SessionStorage for Arc<T> {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        (**self).write(id, events)
    }
    fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        (**self).load(id)
    }
    fn list(&self) -> Result<Vec<SessionId>> {
        (**self).list()
    }
    fn prune(&self, keep: usize) -> Result<usize> {
        (**self).prune(keep)
    }
}

// ── Hook ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HookContext {
    pub session_id: SessionId,
    pub tool_call: ToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookVerdict {
    Allow,
    Deny { reason: String, remediation: String },
}

pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    fn post_execute(&self, _ctx: &HookContext, _result: &ToolResult) {}
}
```

- [ ] **Step 4: Verify braid-ports compiles in isolation**

```bash
cargo check -p braid-ports
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-ports/ Cargo.toml Cargo.lock
git commit -m "feat(braid-ports): add inner-ring port traits crate"
```

---

## Task 2: Update `braid-core`

**Files:**
- Modify: `crates/braid-core/Cargo.toml`
- Modify: `crates/braid-core/src/lib.rs`
- Modify: `crates/braid-core/src/tools.rs`
- Modify: `crates/braid-core/src/registry.rs`
- Modify: `crates/braid-core/src/engine.rs`

- [ ] **Step 1: Add braid-ports dep to braid-core**

Edit `crates/braid-core/Cargo.toml`:

```toml
[package]
name = "braid-core"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-ports = { path = "../braid-ports" }
```

- [ ] **Step 2: Update tools.rs — drop ToolExecutor definition, keep StaticTool**

Replace the full contents of `crates/braid-core/src/tools.rs`:

```rust
use anyhow::Result;
use braid_model::{ToolCall, ToolResult};

pub use braid_ports::ToolExecutor;

#[derive(Debug, Clone)]
pub struct StaticTool {
    name: String,
    output: String,
}

impl StaticTool {
    pub fn new(name: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            output: output.into(),
        }
    }
}

impl ToolExecutor for StaticTool {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        Ok(ToolResult {
            name: call.name.if_empty_then(self.name.clone()),
            output: self.output.clone(),
        })
    }
}

trait StringFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}
```

- [ ] **Step 3: Update registry.rs — change ToolExecutor import**

Change line 1 of `crates/braid-core/src/registry.rs` from:

```rust
use crate::tools::ToolExecutor;
```

to:

```rust
use braid_ports::ToolExecutor;
```

- [ ] **Step 4: Write the new engine.rs**

Replace the full contents of `crates/braid-core/src/engine.rs`:

```rust
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
        Self { provider, tool_executor, event_sink, redactor }
    }

    pub fn run(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput> {
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
                    state.messages.push(tool_result_to_message(&call, &result));
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
            Self { call_count: std::cell::Cell::new(0) }
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
                        content: vec![ContentPart::Text { text: "done after tool".into() }],
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
                        content: vec![ContentPart::Text { text: "hello".into() }],
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
        // SessionStarted, ProviderResponded, SessionCompleted
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
                        content: vec![ContentPart::Text { text: "use a tool".into() }],
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
        // SessionStarted, ProviderResponded (1st), ToolCalled, ToolCompleted,
        // ProviderResponded (2nd), SessionCompleted
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
        assert!(matches!(events.last().unwrap().kind, EventKind::SessionCompleted));
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
                        content: vec![ContentPart::Text { text: format!("saw: {text}") }],
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
                        content: vec![ContentPart::Text { text: "my secret key".into() }],
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
        assert!(!response_text.contains("secret"), "raw secret should not reach provider");
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
```

- [ ] **Step 5: Update lib.rs to re-export port traits**

Replace `crates/braid-core/src/lib.rs`:

```rust
pub mod engine;
pub mod planner;
pub mod registry;
pub mod tools;

// Re-export port traits at crate root for backward compatibility
pub use braid_ports::{EventSink, Provider, Redactor, ToolExecutor};
pub use engine::{Engine, RunInput, RunOutput};
pub use planner::{Action, Planner, SessionState, SimpleLoopPlanner};
pub use registry::ToolRegistry;
pub use tools::StaticTool;
```

- [ ] **Step 6: Run braid-core tests**

```bash
cargo nextest run -p braid-core
```

Expected: all tests pass (no failures). Fix any compilation errors before proceeding.

- [ ] **Step 7: Commit**

```bash
git add crates/braid-core/
git commit -m "feat(braid-core): Engine<P,T,S,R> with EventSink+Redactor ports; drop RunOutput.events"
```

---

## Task 3: Update `braid-providers`

**Files:**
- Modify: `crates/braid-providers/Cargo.toml`
- Modify: `crates/braid-providers/src/openai.rs`
- Modify: `crates/braid-providers/src/lib.rs`
- Create: `crates/braid-providers/src/mock.rs`

- [ ] **Step 1: Swap deps in Cargo.toml**

Replace `crates/braid-providers/Cargo.toml`:

```toml
[package]
name = "braid-providers"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[features]
test-support = []

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-ports = { path = "../braid-ports" }
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde_json.workspace = true
```

- [ ] **Step 2: Update Provider import in openai.rs**

In `crates/braid-providers/src/openai.rs`, change the import of `Provider` from:

```rust
use braid_core::Provider;
```

to:

```rust
use braid_ports::Provider;
```

(Search for whatever the current import is — it likely imports from `braid_core` or `braid_core::engine`. Update it to `braid_ports::Provider`.)

- [ ] **Step 3: Write failing test for MockProvider**

Add to the end of `crates/braid-providers/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    #[cfg(feature = "test-support")]
    #[test]
    fn mock_provider_returns_configured_response() {
        use crate::MockProvider;
        use braid_model::{ContentPart, Message, ProviderRequest, Role};
        use braid_ports::Provider;

        let provider = MockProvider::with_text("hello from mock");
        let req = ProviderRequest {
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: "hi".into() }],
            }],
            tools: vec![],
        };
        let resp = provider.complete(req).unwrap();
        let text = match &resp.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(text, "hello from mock");
    }
}
```

- [ ] **Step 4: Verify test is gated (not yet runnable)**

```bash
cargo nextest run -p braid-providers --features test-support 2>&1 | head -20
```

Expected: zero tests collected (the `#[cfg(feature = "test-support")]` block compiles, but `MockProvider` is not yet defined so it's a compilation error when the feature is active). The feature gate means without it the test is silently skipped — not a true TDD red. Proceed directly to implementation.

- [ ] **Step 5: Create mock.rs**

Create `crates/braid-providers/src/mock.rs`:

```rust
use anyhow::Result;
use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role};
use braid_ports::Provider;

/// A test double for `Provider` that returns a fixed text response.
pub struct MockProvider {
    response_text: String,
}

impl MockProvider {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self { response_text: text.into() }
    }
}

impl Provider for MockProvider {
    fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text {
                    text: self.response_text.clone(),
                }],
            },
            token_count: None,
        })
    }
}
```

- [ ] **Step 6: Wire MockProvider in lib.rs**

Replace `crates/braid-providers/src/lib.rs`:

```rust
pub mod openai;

#[cfg(feature = "test-support")]
pub mod mock;

pub use openai::OpenAiProvider;

#[cfg(feature = "test-support")]
pub use mock::MockProvider;
```

- [ ] **Step 7: Run tests**

```bash
cargo nextest run -p braid-providers --features test-support
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/braid-providers/
git commit -m "feat(braid-providers): import Provider from braid-ports; add MockProvider"
```

---

## Task 4: Update `braid-hooks`

**Files:**
- Modify: `crates/braid-hooks/Cargo.toml`
- Modify: `crates/braid-hooks/src/contract.rs`
- Modify: `crates/braid-hooks/src/executor.rs`
- Modify: `crates/braid-hooks/src/lib.rs`

- [ ] **Step 1: Swap deps in Cargo.toml**

Replace `crates/braid-hooks/Cargo.toml`:

```toml
[package]
name = "braid-hooks"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-ports = { path = "../braid-ports" }
```

Note: `braid-core` dep is dropped entirely. `braid-model` **must remain a direct dep** — `registry.rs` imports `braid_model` types directly, and relying on transitive deps is fragile under Cargo resolver v2.

- [ ] **Step 2: Replace contract.rs with re-exports from braid-ports**

Replace `crates/braid-hooks/src/contract.rs` entirely:

```rust
// Hook, HookContext, HookVerdict have moved to braid-ports.
// Re-export them here for backward compatibility with any code that
// imports from braid_hooks::contract.
pub use braid_ports::{Hook, HookContext, HookVerdict};
```

- [ ] **Step 3: Update executor.rs — change ToolExecutor import**

In `crates/braid-hooks/src/executor.rs`, change:

```rust
use braid_core::ToolExecutor;
```

to:

```rust
use braid_ports::ToolExecutor;
```

Also change the `StaticTool` import in the test from `braid_core::StaticTool` to `braid_model` types or inline a minimal stub. Since `StaticTool` lives in `braid-core` and `braid-hooks` no longer depends on `braid-core`, update the test to use an inline stub:

In the `#[cfg(test)]` block of `executor.rs`, replace `use braid_core::StaticTool;` with an inline test double:

```rust
// Replace: use braid_core::StaticTool;
// With:
struct FixedTool(&'static str);
impl ToolExecutor for FixedTool {
    fn execute(&self, call: braid_model::ToolCall) -> anyhow::Result<braid_model::ToolResult> {
        Ok(braid_model::ToolResult { name: call.name, output: self.0.into() })
    }
}
```

Then replace `StaticTool::new("echo", "hello")` with `FixedTool("hello")` and `StaticTool::new("shell", "output")` with `FixedTool("output")` and `StaticTool::new("echo", "echoed output")` with `FixedTool("echoed output")` in the test bodies.

The `braid_model` import is available transitively through `braid-ports`.

- [ ] **Step 4: Update lib.rs re-exports**

`lib.rs` can stay as-is — `pub use contract::{Hook, HookContext, HookVerdict}` now resolves through the thin re-export wrapper in `contract.rs` to `braid-ports`. Verify the chain compiles:

```bash
cargo check -p braid-hooks
```

Expected: clean.

- [ ] **Step 5: Run tests**

```bash
cargo nextest run -p braid-hooks
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-hooks/
git commit -m "feat(braid-hooks): import Hook/ToolExecutor from braid-ports; drop braid-core dep"
```

---

## Task 5: Update `braid-redact`

**Files:**
- Modify: `crates/braid-redact/Cargo.toml`
- Modify: `crates/braid-redact/src/pipeline.rs`

- [ ] **Step 1: Add braid-ports dep**

Edit `crates/braid-redact/Cargo.toml` — add `braid-ports`:

```toml
[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-ports = { path = "../braid-ports" }
regex = { workspace = true }
serde_json.workspace = true
```

- [ ] **Step 2: Write failing test for Redactor impl**

Add to the end of `crates/braid-redact/src/pipeline.rs` tests block:

```rust
#[test]
fn redaction_pipeline_implements_redactor_port() {
    use braid_ports::Redactor;
    use braid_model::{ContentPart, Message, Role};
    use crate::patterns::SecretPatternRule;

    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

    // Redactor trait method
    let msg = Message {
        role: Role::User,
        content: vec![ContentPart::Text {
            text: "key: sk-abcdefghijklmnopqrstuvwxyz".into(),
        }],
    };
    let redacted = <RedactionPipeline as Redactor>::redact_message(&pipeline, &msg);
    match &redacted.content[0] {
        ContentPart::Text { text } => {
            assert!(text.contains("[REDACTED:api-key]"));
            assert!(!text.contains("sk-"));
        }
        _ => panic!("expected Text"),
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo nextest run -p braid-redact -- redaction_pipeline_implements_redactor_port
```

Expected: compilation error — `Redactor` trait not implemented for `RedactionPipeline`.

- [ ] **Step 4: Add Redactor impl to pipeline.rs**

Add after the `impl Default for RedactionPipeline` block in `crates/braid-redact/src/pipeline.rs`:

```rust
impl braid_ports::Redactor for RedactionPipeline {
    fn redact_message(&self, msg: &braid_model::Message) -> braid_model::Message {
        self.redact_message(msg)
    }
}
```

- [ ] **Step 5: Run all redact tests**

```bash
cargo nextest run -p braid-redact
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-redact/
git commit -m "feat(braid-redact): implement Redactor port on RedactionPipeline"
```

---

## Task 6: Update `braid-observe`

**Files:**
- Modify: `crates/braid-observe/Cargo.toml`
- Modify: `crates/braid-observe/src/store.rs`
- Create: `crates/braid-observe/src/memory.rs`
- Modify: `crates/braid-observe/src/lib.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace `crates/braid-observe/Cargo.toml`:

```toml
[package]
name = "braid-observe"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[features]
test-support = []

[dependencies]
anyhow.workspace = true
braid-model = { path = "../braid-model" }
braid-ports = { path = "../braid-ports" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Write failing tests for EventSink and SessionStorage impls**

Add to `crates/braid-observe/src/store.rs` tests block:

```rust
#[test]
fn session_store_implements_event_sink() {
    use braid_ports::{EventSink, SessionStorage};
    use braid_model::{Event, EventKind, SessionId};
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(SessionStore::open(dir.path().to_path_buf()).unwrap());
    let id = SessionId("sink-test".into());

    // Record some events via EventSink
    store.record(&Event { session_id: id.clone(), kind: EventKind::SessionStarted }).unwrap();
    store.record(&Event { session_id: id.clone(), kind: EventKind::SessionCompleted }).unwrap();

    // Flush persists them
    store.flush().unwrap();

    // Load via SessionStorage trait (imported above) to verify
    // Arc<SessionStore> implements SessionStorage via braid-ports blanket impl
    let loaded = store.load(&id).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].kind, EventKind::SessionStarted);
}

#[test]
fn session_store_implements_session_storage() {
    use braid_ports::SessionStorage;
    use braid_model::{Event, EventKind, SessionId};

    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let id = SessionId("storage-test".into());
    let events = make_events("storage-test");

    // Use trait methods via UFCS to confirm trait impl exists
    <SessionStore as SessionStorage>::write(&store, &id, &events).unwrap();
    let loaded = <SessionStore as SessionStorage>::load(&store, &id).unwrap();
    assert_eq!(loaded, events);
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo nextest run -p braid-observe -- session_store_implements_event_sink session_store_implements_session_storage
```

Expected: compilation error — trait impls missing.

- [ ] **Step 4: Add buffer field and EventSink + SessionStorage impls to store.rs**

At the top of `crates/braid-observe/src/store.rs`, update the imports:

```rust
use anyhow::Result;
use braid_model::{Event, SessionId};
use braid_ports::{EventSink, SessionStorage};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Mutex;
```

Change `SessionStore` struct to add `buffer`:

```rust
pub struct SessionStore {
    root: PathBuf,
    buffer: Mutex<Vec<Event>>,
}
```

Update `SessionStore::open` to initialize the buffer:

```rust
pub fn open(root: PathBuf) -> Result<Self> {
    fs::create_dir_all(&root)?;
    Ok(Self { root, buffer: Mutex::new(vec![]) })
}
```

Add the trait impls after all inherent methods (before the test module):

```rust
impl EventSink for SessionStore {
    fn record(&self, event: &Event) -> Result<()> {
        self.buffer.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        let events = {
            let mut buf = self.buffer.lock().unwrap();
            std::mem::take(&mut *buf)
        };
        if events.is_empty() {
            return Ok(());
        }
        let session_id = &events[0].session_id;
        self.write(session_id, &events)
    }
}

impl Drop for SessionStore {
    fn drop(&mut self) {
        // Best-effort flush on drop — primary flush path is explicit flush() call.
        let _ = self.flush();
    }
}

impl SessionStorage for SessionStore {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        self.write(id, events)
    }
    fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        self.load(id)
    }
    fn list(&self) -> Result<Vec<SessionId>> {
        self.list()
    }
    fn prune(&self, keep: usize) -> Result<usize> {
        self.prune(keep)
    }
}
```

Note: `SessionStorage::write` delegates to inherent `self.write`. This compiles because inherent methods are resolved before trait methods when using `self.write(...)` inside the trait impl.

- [ ] **Step 5: Verify the new tests fail (impls not yet written)**

```bash
cargo nextest run -p braid-observe -- session_store_implements_event_sink session_store_implements_session_storage
```

Expected: compilation error — `EventSink` / `SessionStorage` not implemented for `SessionStore`.

- [ ] **Step 6: Write failing test for InMemorySessionStorage**

Create `crates/braid-observe/src/memory.rs`:

```rust
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
            self.sessions.lock().unwrap().insert(id.0.clone(), events.to_vec());
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
            Event { session_id: SessionId(id.into()), kind: EventKind::SessionStarted }
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
```

- [ ] **Step 7: Update lib.rs**

Replace `crates/braid-observe/src/lib.rs`:

```rust
pub mod render;
pub mod store;

#[cfg(feature = "test-support")]
pub mod memory;

pub use render::render_session;
pub use store::{SessionMeta, SessionStore};

#[cfg(feature = "test-support")]
pub use memory::InMemorySessionStorage;
```

- [ ] **Step 8: Run all observe tests**

```bash
cargo nextest run -p braid-observe --features test-support
```

Expected: all tests pass. The existing tests (writes_and_loads_roundtrip, etc.) should still pass — the `buffer` field is new but doesn't affect the existing inherent `write` method.

- [ ] **Step 9: Commit**

```bash
git add crates/braid-observe/
git commit -m "feat(braid-observe): implement EventSink+SessionStorage ports; add InMemorySessionStorage"
```

---

## Task 7: Update `braid-cli`

**Files:**
- Modify: `crates/braid-cli/src/main.rs`

This is the composition root. The goal is to wire `Engine<P, T, S, R>` explicitly, drop `with_redactor`, use `Arc<SessionStore>` as the shared event sink, and remove the now-dead post-run redaction loop.

- [ ] **Step 1: Update imports at the top of main.rs**

Replace the existing import block:

```rust
use std::io::{self, IsTerminal, Read};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use braid_core::{Engine, RunInput, SimpleLoopPlanner, ToolRegistry};
use braid_hooks::{DestructiveCommandGuard, HookRegistry, HookedExecutor};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_observe::SessionStore;
use braid_ports::Provider;
use braid_providers::OpenAiProvider;
use braid_redact::{EnvVarRule, HomePathRule, RedactionPipeline, SecretPatternRule};
```

- [ ] **Step 2: Rewrite cmd_run**

Replace the `fn cmd_run` body:

```rust
fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let redactor = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let session_id = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        SessionId(format!("{secs}"))
    };

    // Arc lets cmd_sessions (and any future caller) share the same store instance.
    let store = Arc::new(SessionStore::open(default_store_dir()?)?);

    let hooks = HookRegistry::fail_closed().register(DestructiveCommandGuard::new());
    let tools = HookedExecutor::new(ToolRegistry::new(), hooks, session_id.clone());

    let engine = Engine::new(provider, tools, Arc::clone(&store), redactor);
    let output = engine.run(
        RunInput {
            session_id,
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
    println!("{response_text}");
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}
```

- [ ] **Step 3: Update cmd_sessions to bring SessionStorage trait into scope**

`cmd_sessions` opens its own `SessionStore` directly (safe second open — no event buffer in use). Per the spec, it should use the `SessionStorage` trait where possible. Add `use braid_ports::SessionStorage;` to the imports at the top of `main.rs`, then the existing `store.list()`, `store.load(...)`, and `store.prune(...)` calls in `cmd_sessions` resolve through the trait. `store.load_meta(...)` remains an inherent-method call (not part of the trait) — no change needed there.

- [ ] **Step 4: Update doctor module**

In the `doctor` module, find `use braid_core::engine::Provider;` and change it to `use braid_ports::Provider;`. The doctor module calls `provider.complete(...)` — the trait now comes from `braid-ports` but the functionality is identical.

- [ ] **Step 5: Update braid-cli Cargo.toml to add braid-hooks and braid-ports**

Check `crates/braid-cli/Cargo.toml`. It needs `braid-hooks` and `braid-ports` as direct deps since `cmd_run` now uses `HookRegistry`, `HookedExecutor`, and `DestructiveCommandGuard` directly. Add them if missing:

```toml
braid-hooks = { path = "../braid-hooks" }
braid-ports = { path = "../braid-ports" }
```

- [ ] **Step 6: Run braid-cli check**

```bash
cargo check -p braid-cli
```

Expected: no errors. Fix any remaining import issues.

- [ ] **Step 7: Run workspace tests**

```bash
cargo nextest run --workspace
```

Expected: all tests pass across all crates.

- [ ] **Step 8: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings before committing.

- [ ] **Step 9: Commit**

```bash
git add crates/braid-cli/
git commit -m "feat(braid-cli): wire Engine<P,T,S,R> with Arc<SessionStore> as EventSink"
```

---

## Task 8: Final verification

- [ ] **Step 1: Run full workspace check**

```bash
just check
```

Expected: clean.

- [ ] **Step 2: Run full test suite**

```bash
just test
```

Expected: all tests pass.

- [ ] **Step 3: Run clippy clean**

```bash
just clippy
```

Expected: zero warnings.

- [ ] **Step 4: Verify dep graph — no adapter depends on another adapter**

```bash
cargo tree -p braid-hooks 2>&1 | head -20
cargo tree -p braid-redact 2>&1 | head -20
cargo tree -p braid-providers 2>&1 | head -20
cargo tree -p braid-observe 2>&1 | head -20
```

Expected: none of the above show `braid-core` in their direct deps. Each shows `braid-ports` (and `braid-model` transitively).

- [ ] **Step 5: Commit final state**

```bash
git add -A
git commit -m "chore: hexagonal refactor complete — braid-ports as inner ring"
```
