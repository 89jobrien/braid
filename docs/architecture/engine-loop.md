# Engine Loop

`Engine<T, P>` in `braid-core` is the runtime heart of braid. It drives the agent loop via a pluggable `Planner`.

## Engine Structure

```rust
Engine<T: ToolExecutor, P: Provider> {
    tool_executor: T,
    provider: P,
    redactor: Option<Box<dyn Fn(&Message) -> Message>>,
    event_callback: Option<Box<dyn Fn(&Event)>>,
}
```

Builder methods (all return `Self` for chaining):

| Method | Effect |
|---|---|
| `Engine::new(tool_executor, provider)` | Create with no redactor, no callback |
| `.with_redactor(fn)` | Apply `fn` to all messages before each provider call |
| `.with_event_callback(fn)` | Call `fn` synchronously for each emitted event |

## Run Method

`Engine::run(input: RunInput, planner: &impl Planner) -> Result<RunOutput>`

### Inputs

```rust
RunInput {
    session_id: SessionId,
    messages: Vec<Message>,   // initial conversation (typically one User message)
    max_turns: Option<u32>,   // None = use planner default (20)
}
```

### Outputs

```rust
RunOutput {
    provider_response: ProviderResponse,  // final LLM response
    events: Vec<Event>,                   // all events emitted during the run
}
```

## The Loop

```
emit SessionStarted
│
├─ initialize SessionState { messages, pending_tool_calls: [], turn_count: 0, max_turns }
│
└─ loop:
    │
    ├─ planner.next_action(&state) → Action
    │
    ├─ Action::CallProvider { messages }:
    │   │  apply redactor to each message
    │   │  provider.complete(ProviderRequest { redacted_messages, tools: [] })
    │   │    → ProviderResponse
    │   │  update state: append response message, extract ToolUse parts → pending_tool_calls
    │   └─ emit ProviderResponded
    │
    ├─ Action::ExecuteTool { call: ToolCall }:
    │   │  emit ToolCalled { tool_name }
    │   │  tool_executor.execute(call) → ToolResult
    │   │  update state: remove call from pending, append ToolResult message
    │   └─ emit ToolCompleted { tool_name }
    │
    └─ Action::Finish { response }:
        └─ break loop, return response

emit SessionCompleted
return RunOutput
```

## SimpleLoopPlanner Decision Logic

`SimpleLoopPlanner` implements the default agent loop:

```
next_action(state):
    if turn_count >= max_turns → Finish (with last provider response)
    if turn_count > 0 AND no last_provider_response → Error (stuck)
    if pending_tool_calls is non-empty → ExecuteTool (first in list)
    if last response has NO ToolUse content → Finish
    else → CallProvider (with current messages)
```

A typical minimal session (one-shot prompt, no tools):

```
SessionStarted
  turn 0: CallProvider → ProviderResponded  (response has no ToolUse)
  turn 0: Finish
SessionCompleted
```

A session with one tool call:

```
SessionStarted
  turn 0: CallProvider → ProviderResponded  (response has ToolUse)
  turn 0: ExecuteTool  → ToolCalled, ToolCompleted
  turn 1: CallProvider → ProviderResponded  (response has no ToolUse)
  turn 1: Finish
SessionCompleted
```

## Event Emission

Every `events.push(...)` call goes through the `emit!` macro:

```rust
macro_rules! emit {
    ($event:expr) => {{
        let event = $event;
        if let Some(cb) = &self.event_callback {
            cb(&event);         // synchronous — fires before push
        }
        events.push(event);
    }};
}
```

The callback fires **before** the event is appended to `RunOutput.events`. This allows `cmd_run` to stream events to disk incrementally without waiting for the session to complete.

## SessionState

The planner sees this view of the running session:

```rust
SessionState {
    messages: Vec<Message>,
    pending_tool_calls: Vec<ToolCall>,
    last_provider_response: Option<ProviderResponse>,
    turn_count: u32,
    max_turns: u32,
}
```

`turn_count` increments on every `CallProvider`. `pending_tool_calls` is populated from `ToolUse` content parts in the provider response and drained one-at-a-time by `ExecuteTool`.

## Extending the Engine

### Custom Planner

Implement `Planner` to change the loop strategy. Examples: multi-agent delegation, budget-aware planning, step-by-step verification.

### Custom ToolExecutor

Implement `ToolExecutor` or compose via `HookedExecutor<T>`. The engine never inspects tool names — all dispatch is in the executor.

### Custom Provider

Implement `Provider` for any LLM backend. The engine calls `complete()` and expects a `ProviderResponse`.
