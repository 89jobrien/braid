# Design: Planner/Executor Separation

**Date**: 2026-03-15
**Status**: Approved
**Scope**: Phase 1 — separate planning decisions from execution in braid-core's engine loop

---

## 1. Planner Trait (braid-core)

### Action Enum

```rust
pub enum Action {
    CallProvider { messages: Vec<Message> },
    ExecuteTool { call: ToolCall },
    Finish { response: ProviderResponse },
}
```

### Planner Trait

```rust
pub trait Planner {
    fn next_action(&self, state: &SessionState) -> Result<Action>;
}
```

The planner inspects the current session state and decides what to do next. It never executes anything — the engine does that.

---

## 2. SessionState (braid-core)

Tracks in-progress session context for the planner to inspect:

```rust
pub struct SessionState {
    pub messages: Vec<Message>,
    pub pending_tool_calls: Vec<ToolCall>,
    pub last_provider_response: Option<ProviderResponse>,
    pub turn_count: u32,
    pub max_turns: u32,
}
```

- `messages`: full conversation history (user messages, assistant responses, tool results)
- `pending_tool_calls`: tool calls from the last provider response not yet executed
- `last_provider_response`: most recent provider response (for `Finish` when done)
- `turn_count`: number of provider calls made so far
- `max_turns`: limit to prevent runaway loops

---

## 3. SimpleLoopPlanner (braid-core)

Built-in default planner implementing the standard tool-call loop:

1. If `turn_count >= max_turns`, return `Finish` with the last provider response (or error if none).
2. If `pending_tool_calls` is not empty, return `ExecuteTool` with the first pending call.
3. If `last_provider_response` is `Some` and has no tool calls pending, return `Finish`.
4. Otherwise, return `CallProvider` with the current message history.

---

## 4. Engine Changes (braid-core)

### RunInput Changes

```rust
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub max_turns: Option<u32>, // default: 10
}
```

### Engine Type Signature

`Engine<T, P>` becomes `Engine<T, P, L>` where `L: Planner`. Or simpler: the engine takes a planner as a parameter to `run()` rather than a type param. Recommend the simpler approach:

```rust
impl<T, P> Engine<T, P>
where
    T: ToolExecutor,
    P: Provider,
{
    pub fn run(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput>;
}
```

This avoids changing the Engine struct. Callers pass `&SimpleLoopPlanner` by default.

### Engine Loop

```
fn run(&self, input: RunInput, planner: &impl Planner) -> Result<RunOutput> {
    1. Initialize SessionState from input (messages, max_turns, empty pending_tool_calls)
    2. Loop:
       a. let action = planner.next_action(&state)?
       b. match action:
          - CallProvider { messages } → call self.provider.complete(), update state
            - Parse tool calls from response content (ContentPart::ToolUse)
            - Add assistant message to state.messages
            - Set pending_tool_calls from response
            - Increment turn_count
            - Emit ProviderResponded event
          - ExecuteTool { call } → call self.tool_executor.execute(), update state
            - Remove call from pending_tool_calls
            - Add tool result as a Tool-role message to state.messages
            - Emit ToolCalled, ToolCompleted events
          - Finish { response } → return RunOutput
            - Emit SessionCompleted event (new EventKind)
    3. Return RunOutput { provider_response, events }
}
```

### RunOutput

Stays the same: `{ provider_response: ProviderResponse, events: Vec<Event> }`.

---

## 5. Model Changes (braid-model)

### Rename SessionState → SessionPhase

The existing `SessionState` enum in `braid-model/src/session.rs` is renamed to `SessionPhase` to avoid collision with the new `SessionState` struct in braid-core.

### New EventKind Variants

Add to `EventKind`:
- `ToolCalled { tool_name: String }` — already exists
- `ToolCompleted { tool_name: String }` — already exists
- `SessionCompleted` — new, emitted when the loop finishes

### Tool Call Extraction

The engine needs to extract `ToolCall` values from `ContentPart::ToolUse` in a provider response. This is a mapping function, not a new type:

```rust
fn extract_tool_calls(message: &Message) -> Vec<ToolCall> {
    message.content.iter().filter_map(|part| {
        match part {
            ContentPart::ToolUse { id, name, input } => Some(ToolCall {
                name: name.clone(),
                input: input.to_string(),
            }),
            _ => None,
        }
    }).collect()
}
```

### Tool Result to Message

When a tool completes, its result needs to become a `Message` for the conversation:

```rust
fn tool_result_to_message(call: &ToolCall, result: &ToolResult) -> Message {
    Message {
        role: Role::Tool,
        content: vec![ContentPart::ToolResult {
            tool_use_id: /* needs the tool call id */,
            content: result.output.clone(),
        }],
    }
}
```

**Issue**: `ToolCall` currently has `name` and `input` but no `id`. The `id` is needed to correlate tool results with tool calls in the OpenAI API. `ContentPart::ToolUse` has the `id` field. The engine should pass the `id` through from `ContentPart::ToolUse` when building `ToolCall`.

**Fix**: Either add an `id` field to `ToolCall` in braid-model, or have the engine track the correlation separately. Adding `id: String` to `ToolCall` is cleaner.

---

## 6. Dependencies

No new crate dependencies. All changes are in braid-core and braid-model (stdlib only).

---

## 7. Build Order

1. Rename `SessionState` → `SessionPhase` in braid-model, update all references
2. Add `id: String` field to `ToolCall`, add `SessionCompleted` to `EventKind`
3. Add `Action`, `Planner` trait, `SessionState` struct to braid-core
4. Implement `SimpleLoopPlanner`
5. Rewrite `Engine::run` as a loop using the planner
6. Update CLI to pass `SimpleLoopPlanner` and `max_turns`
7. Update tests
