# braid-engine

`braid-engine` runs Braid's synchronous provider/tool loop. It coordinates provider completions,
tool calls, context injection, event recording, and message redaction through traits from
`braid-ports`.

## Workspace role and architecture

The engine is the application core: it depends only on `braid-model` and `braid-ports`, not on any
provider or UI adapter. Its generic `Engine<P, T, S, R, C>` accepts implementations of `Provider`,
`ToolExecutor`, `EventSink`, `Redactor`, and optionally `ContextProvider`.

`SimpleLoopPlanner` selects the next action from `SessionState`:

1. Finish with the last response when the turn limit is reached.
2. Execute the first pending tool call.
3. Finish after a provider response with no tool calls.
4. Otherwise call the provider again.

## Key APIs

| API | Purpose |
| --- | --- |
| `Engine::new` | Construct the engine from provider, tool, sink, and redactor adapters |
| `Engine::with_context` | Add a context provider that injects a leading system message |
| `Engine::run` | Run a session and flush the event sink |
| `RunInput`, `RunOutput` | Session input and final provider response |
| `Planner`, `Action`, `SessionState` | Custom planning contract and state |
| `SimpleLoopPlanner` | Default provider/tool loop |
| `ToolRegistry` | Name-based dispatch for multiple `ToolExecutor` implementations |
| `StaticTool` | Fixed-output tool useful in examples and tests |

The crate re-exports `Provider`, `ToolExecutor`, `EventSink`, and `Redactor` for backward
compatibility.

## Usage

```rust
use anyhow::Result;
use braid_engine::{Engine, RunInput, SimpleLoopPlanner, StaticTool};
use braid_model::{ContentPart, Event, Message, ProviderRequest, ProviderResponse, Role, SessionId};
use braid_ports::{EventSink, Provider, Redactor};

struct FixedProvider;
impl Provider for FixedProvider {
    fn complete(&self, _: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text { text: "done".into() }],
            },
            token_count: None,
        })
    }
}

struct NoEvents;
impl EventSink for NoEvents {
    fn record(&self, _: &Event) -> Result<()> { Ok(()) }
}

struct Passthrough;
impl Redactor for Passthrough {
    fn redact_message(&self, message: &Message) -> Message { message.clone() }
}

let engine = Engine::new(FixedProvider, StaticTool::new("echo", "ok"), NoEvents, Passthrough);
let output = engine.run(
    RunInput {
        session_id: SessionId("example".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: "hello".into() }],
        }],
        max_turns: Some(3),
    },
    &SimpleLoopPlanner,
)?;
# Ok::<(), anyhow::Error>(())
```

## Runtime behavior

The engine emits `SessionStarted`, `ProviderResponded`, `ToolCalled`, `ToolCompleted`, and
`SessionCompleted`. Messages are redacted before provider calls, and tool-result messages are
redacted before being appended to session state. Context assembly failure is non-fatal.

The current provider request from the engine passes an empty tool-definition list. Provider tool
calls are still recognized when returned as `ContentPart::ToolUse`.

## Implementation status

The default loop, custom planner port, tool dispatch, context injection, redaction, event emission,
turn limits, and error-path flushing are implemented. `NoopContextProvider` backs engines created
without `with_context`.

## Features

This crate has no Cargo feature flags.

## Development and testing

Unit tests cover minimal sessions, tool loops, turn limits, redaction, planner decisions, and tool
registry dispatch.

```console
cargo nextest run -p braid-engine
cargo clippy -p braid-engine -- -D warnings
cargo check -p braid-engine
```
