# Hexagonal Refactor Design

**Date:** 2026-03-25
**Status:** Draft
**Scope:** Full workspace refactor — all 8 crates

## Problem

The braid workspace has a strong foundation (domain types isolated in `braid-model`, generic `Engine<T, P>`, trait-based providers and executors) but three gaps prevent it from being fully hexagonal:

1. **Missing ports:** `SessionStore` is a concrete filesystem struct with no trait. Events are accumulated in `RunOutput` rather than flowing through a port.
2. **Trait scatter:** `Provider` and `ToolExecutor` live in `braid-core`; `RedactionRule` in `braid-redact`; `Hook` in `braid-hooks`. No single place defines "what are the ports of this system."
3. **Awkward dep:** `braid-hooks` depends on `braid-core` only to get the `ToolExecutor` trait, creating a conceptually backwards coupling.

**Goals:** Testability (swap in-memory doubles everywhere), swappability (one-line provider/storage changes), boundary clarity (the dep graph communicates architecture).

## Approach: Extract `braid-ports`

A new `braid-ports` crate (deps: `braid-model` only) becomes the inner ring of the hexagon. It holds all port trait definitions. Every adapter crate depends on `braid-ports`, not on each other.

## Dependency Graph

```
braid-model          (domain types — leaf, unchanged)
     ↑
braid-ports          (NEW — all port trait definitions)
     ↑
braid-core           (engine loop + internal strategy traits)
     ↑
braid-providers      (OpenAI adapter)
braid-hooks          (hook adapters — drops braid-core dep)
braid-redact         (redaction adapters — switches from braid-model to braid-ports)
braid-observe        (session storage adapter)
braid-mcp            (MCP server — unchanged)
     ↑
braid-cli            (composition root)
```

Dependencies point inward. No adapter crate depends on another adapter crate. `braid-cli` is the only place that knows about all layers.

## `braid-ports` Contents

All traits that define an external boundary:

```rust
// Provider port — LLM completion
pub trait Provider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

// ToolExecutor port — tool dispatch
pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}

// Redactor port — privacy filter
pub trait Redactor {
    fn redact_message(&self, msg: &Message) -> Message;
}

// EventSink port — observability pipeline
pub trait EventSink {
    fn record(&self, event: &Event) -> Result<()>;
}

// SessionStorage port — durable event store
pub trait SessionStorage {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()>;
    fn load(&self, id: &SessionId) -> Result<Vec<Event>>;
    fn list(&self) -> Result<Vec<SessionId>>;
    fn prune(&self, keep: usize) -> Result<usize>;
}

// Hook port — pre/post tool execution policy
pub trait Hook {
    fn name(&self) -> &str;
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    fn post_execute(&self, ctx: &HookContext, result: &ToolResult) {}
}
```

`Planner` stays in `braid-core` — it is an internal engine strategy (uses `SessionState` and `Action`), not an external boundary.

## `braid-core` Changes

`Engine` gains a fourth generic parameter for the redactor, replacing the `Redactor` closure alias:

```rust
pub struct Engine<P: Provider, T: ToolExecutor, S: EventSink, R: Redactor> {
    provider: P,
    tool_executor: T,
    event_sink: S,
    redactor: R,
}
```

`RunOutput` drops `Vec<Event>` — events are now pushed to the sink as they happen:

```rust
// before
pub struct RunOutput {
    pub provider_response: ProviderResponse,
    pub events: Vec<Event>,
}

// after
pub struct RunOutput {
    pub provider_response: ProviderResponse,
}
```

`braid-core` retains: `Engine`, `RunInput`, `RunOutput`, `SessionState`, `Action`, `Planner`, `SimpleLoopPlanner`, `ToolRegistry`, `StaticTool`.

## New Ports: EventSink and SessionStorage

**EventSink** replaces event accumulation in `RunOutput`. The engine calls `event_sink.record(&event)` at each lifecycle moment (SessionStarted, ProviderResponded, ToolCalled, ToolCompleted, SessionCompleted).

**SessionStorage** is the durable backend. `SessionStore` (filesystem JSONL) in `braid-observe` implements it. The `SessionStore` also implements `EventSink` by buffering events internally and flushing atomically on `Drop` (or explicit `flush()`).

This lets the engine push events in real-time while storage stays atomic.

## Test Doubles

Each port gets an in-memory double, gated behind a `test-support` feature flag:

| Double | Crate | Port |
|---|---|---|
| `VecEventSink` | `braid-ports` | `EventSink` |
| `PassthroughRedactor` | `braid-ports` | `Redactor` |
| `MockProvider` | `braid-providers` | `Provider` |
| `InMemorySessionStorage` | `braid-observe` | `SessionStorage` |

`PassthroughRedactor` is a no-op — callers that don't need redaction use it instead of `Option<Redactor>`.

## Adapter Changes per Crate

**braid-providers:**
- `OpenAiProvider` now implements `Provider` from `braid-ports`
- Drops `braid-core` dep; adds `braid-ports`
- Adds `MockProvider` behind `test-support` feature

**braid-hooks:**
- `Hook` + `HookContext` + `HookVerdict` move to `braid-ports`
- `HookedExecutor<T: ToolExecutor>` now uses `ToolExecutor` from `braid-ports`
- Drops `braid-core` dep entirely; depends only on `braid-ports`
- `DestructiveCommandGuard`, `FreshnessGuard` unchanged

**braid-redact:**
- `RedactionRule` moves to `braid-ports`
- `RedactionPipeline` implements new `Redactor` port
- Drops `braid-model` direct dep; depends on `braid-ports`

**braid-observe:**
- `SessionStore` implements both `SessionStorage` and `EventSink`
- `SessionMeta` and `render_session()` unchanged
- Adds `InMemorySessionStorage` behind `test-support`

**braid-mcp:**
- No changes — `McpToolRegistry` uses a closure executor, no trait dep on `braid-core`

## Composition Root (`braid-cli`)

```rust
// cmd_run() — explicit wiring, no business logic
let redactor = RedactionPipeline::new()
    .with_rule(SecretPatternRule::new())
    .with_rule(EnvVarRule::new())
    .with_rule(HomePathRule::new());

let store = SessionStore::open(default_store_dir()?)?;  // EventSink + SessionStorage

let hooks = HookRegistry::fail_closed()
    .register(DestructiveCommandGuard::new());

let tools = HookedExecutor::new(ToolRegistry::new(), hooks, session_id.clone());

let provider = resolve_provider(provider_flag, model)?;

let engine = Engine::new(provider, tools, store, redactor);
let output = engine.run(RunInput { session_id, messages, max_turns }, &SimpleLoopPlanner)?;
```

Swapping `OpenAiProvider` for `MockProvider` or `SessionStore` for `InMemorySessionStorage` is a one-line change at the composition root.

## Error Handling

All port trait methods return `Result<_>`. Adapters map infrastructure errors to domain errors at the boundary — no infrastructure error types leak through port signatures.

## Testing Strategy

- **Domain/engine unit tests:** Use `MockProvider` + `VecEventSink` + `PassthroughRedactor` + `StaticTool`. No I/O, no network.
- **Adapter tests:** Each adapter tested independently against its port contract. Filesystem tests in `braid-observe`, HTTP tests in `braid-providers` (behind an integration feature).
- **CLI integration tests:** Wire real adapters in a temp dir, invoke `cmd_run`. Already scaffolded in `crates/braid-cli/tests/`.

## What Does Not Change

- `braid-model` — unchanged, still the leaf
- `braid-mcp` — unchanged
- The `Planner` trait and `SimpleLoopPlanner` — stay in `braid-core`
- `ToolRegistry` — stays in `braid-core` as a utility
- The four-phase build order and overall crate naming

## Migration Order

1. Create `braid-ports` with all trait definitions (no impl)
2. Update `braid-core` to import traits from `braid-ports`; add `EventSink` to `Engine`; drop `events` from `RunOutput`
3. Update `braid-providers` to import `Provider` from `braid-ports`; add `MockProvider`
4. Update `braid-hooks` to import `Hook`/`ToolExecutor` from `braid-ports`; drop `braid-core` dep
5. Update `braid-redact` to import `RedactionRule` from `braid-ports`; implement `Redactor`
6. Update `braid-observe` to implement `SessionStorage` and `EventSink`; add `InMemorySessionStorage`
7. Update `braid-cli` to wire new `Engine<P, T, S, R>` signature
8. Update `Cargo.toml` files to reflect new dep graph
