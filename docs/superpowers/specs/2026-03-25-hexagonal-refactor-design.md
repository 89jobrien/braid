# Hexagonal Refactor Design

**Date:** 2026-03-25
**Status:** Draft
**Scope:** Full workspace refactor — all 8 crates

## Problem

The braid workspace has a strong foundation (domain types isolated in `braid-model`, generic `Engine<T, P>`, trait-based providers and executors) but three gaps prevent it from being fully hexagonal:

1. **Missing ports:** `SessionStore` is a concrete filesystem struct with no trait. Events are accumulated in `RunOutput` rather than flowing through a port.
2. **Trait scatter:** `Provider` and `ToolExecutor` live in `braid-core`; `Hook` in `braid-hooks`. No single place defines "what are the ports of this system."
3. **Awkward dep:** `braid-hooks` depends on `braid-core` only to get the `ToolExecutor` trait, creating a conceptually backwards coupling.

**Goals:** Testability (swap in-memory doubles everywhere), swappability (one-line provider/storage changes), boundary clarity (the dep graph communicates architecture).

## Approach: Extract `braid-ports`

A new `braid-ports` crate (deps: `braid-model` only) becomes the inner ring of the hexagon. It holds all port trait definitions. Every adapter crate depends on `braid-ports`, not on each other.

## Dependency Graph

```
braid-model          (domain types — leaf, unchanged)
     ↑
braid-ports          (NEW — all port trait definitions, deps: braid-model only)
     ↑
braid-core           (engine loop + internal strategy traits)
     ↑
braid-providers      (OpenAI adapter, deps: braid-ports)
braid-hooks          (hook adapters, deps: braid-ports — drops braid-core dep)
braid-redact         (redaction adapters, deps: braid-ports + braid-model)
braid-observe        (session storage adapter, deps: braid-ports + braid-model)
braid-mcp            (MCP server, deps: braid-model only — unchanged)
     ↑
braid-cli            (composition root)
```

Dependencies point inward. No adapter crate depends on another adapter crate. `braid-cli` is the only place that knows about all layers. `braid-mcp` is unchanged — it depends only on `braid-model` and gains no `braid-ports` dep.

## `braid-ports` Contents

All traits that define an external boundary. `braid-ports` contains **only trait definitions and supporting types** — no implementations.

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
// Uses &self with interior mutability in implementations (see EventSink section).
pub trait EventSink {
    fn record(&self, event: &Event) -> Result<()>;
    fn flush(&self) -> Result<()> { Ok(()) }  // default no-op; override for buffered impls
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

// Supporting types that move with Hook
pub struct HookContext { pub session_id: SessionId, pub tool_call: ToolCall }
pub enum HookVerdict { Allow, Deny { reason: String, remediation: String } }
```

`Planner` stays in `braid-core` — it is an internal engine strategy (uses `SessionState` and `Action`), not an external boundary.

`RedactionRule` stays in `braid-redact` — it is a single-string-in / string-out pipeline building block, not an external port. The external port is `Redactor` (operates on `Message`).

## `braid-core` Changes

`Engine` gains a fourth generic parameter for the redactor, replacing the `Redactor` closure alias. Parameter order: `<P: Provider, T: ToolExecutor, S: EventSink, R: Redactor>`.

```rust
pub struct Engine<P: Provider, T: ToolExecutor, S: EventSink, R: Redactor> {
    provider: P,
    tool_executor: T,
    event_sink: S,
    redactor: R,
}

impl<P, T, S, R> Engine<P, T, S, R>
where P: Provider, T: ToolExecutor, S: EventSink, R: Redactor
{
    pub fn new(provider: P, tool_executor: T, event_sink: S, redactor: R) -> Self { ... }
}
```

Note: the existing `Engine::new(tool_executor, provider)` argument order (executor first) is **reversed** to `(provider, tool_executor, event_sink, redactor)`. All existing engine unit tests must be updated.

`RunOutput` drops `Vec<Event>` — events are now pushed to the sink as they happen:

```rust
// before
pub struct RunOutput { pub provider_response: ProviderResponse, pub events: Vec<Event> }

// after
pub struct RunOutput { pub provider_response: ProviderResponse }
```

`braid-core` retains: `Engine`, `RunInput`, `RunOutput`, `SessionState`, `Action`, `Planner`, `SimpleLoopPlanner`, `ToolRegistry`, `StaticTool`. All internal uses of `ToolExecutor` and `Provider` are updated to use the versions from `braid-ports` (via `pub use` re-export or direct import). `ToolRegistry::register()` call sites are unchanged — the trait identity is unified.

`braid-core::ToolExecutor` becomes `pub use braid_ports::ToolExecutor` — not re-declared as a new trait. This is critical: if `braid-core` re-declares `ToolExecutor`, `HookedExecutor<ToolRegistry>` will fail with a trait-identity mismatch at the composition root. Likewise, `braid-core::Provider` becomes `pub use braid_ports::Provider` — not re-declared. `OpenAiProvider` implements `braid_ports::Provider`; `Engine<P: Provider>` must reference the same trait identity.

## New Ports: EventSink and SessionStorage

**EventSink** replaces event accumulation in `RunOutput`. The engine calls `event_sink.record(&event)` at each lifecycle moment (SessionStarted, ProviderResponded, ToolCalled, ToolCompleted, SessionCompleted). At the end of `Engine::run()`, the engine calls `event_sink.flush()` to give buffered implementations a chance to persist.

**Interior mutability:** `EventSink::record()` takes `&self`. Implementations that buffer events (like `SessionStore`) must use `Mutex<Vec<Event>>` internally to satisfy `&self` + `Send`. This is explicitly required by the contract.

**SessionStorage** is the durable backend. `SessionStore` (filesystem JSONL) in `braid-observe` implements it. `SessionStore` also implements `EventSink` by:
1. Buffering events in `Mutex<Vec<Event>>` on `record()`
2. Writing atomically to JSONL + meta on `flush()`
3. Also flushing on `Drop` as a best-effort safety net (not the primary flush path)

This lets the engine push events in real-time while storage stays atomic. Engine always calls `flush()` explicitly at session end — `Drop` is a fallback only.

## Test Doubles

Each port gets an in-memory double, gated behind a `test-support` feature flag:

| Double | Crate | Port |
|---|---|---|
| `VecEventSink` | `braid-core` | `EventSink` |
| `PassthroughRedactor` | `braid-core` | `Redactor` |
| `MockProvider` | `braid-providers` | `Provider` |
| `InMemorySessionStorage` | `braid-observe` | `SessionStorage` |

`VecEventSink` and `PassthroughRedactor` live in `braid-core` (not `braid-ports`) — `braid-ports` contains only trait definitions. `VecEventSink` uses `Mutex<Vec<Event>>` internally for `&self` compatibility.

`PassthroughRedactor` is a no-op — callers that don't need redaction use it instead of `Option<Redactor>`.

## Adapter Changes per Crate

**braid-providers:**
- `OpenAiProvider` now implements `Provider` from `braid-ports`
- Drops `braid-core` dep; adds `braid-ports`
- Adds `MockProvider` behind `test-support` feature

**braid-hooks:**
- `Hook`, `HookContext`, `HookVerdict` move to `braid-ports`
- `HookedExecutor<T: ToolExecutor>` now uses `ToolExecutor` from `braid-ports`
- Drops `braid-core` dep entirely; depends only on `braid-ports`
- `DestructiveCommandGuard` and `FreshnessGuard` unchanged (move with the crate, still implement `Hook`)

**braid-redact:**
- `RedactionRule` stays in `braid-redact` (internal pipeline building block, not a port)
- `RedactionPipeline` implements the new `Redactor` port from `braid-ports`
- Adds `braid-ports` dep; retains `braid-model` dep (needed for `Message`, `ContentPart`, `Event`)

**braid-observe:**
- `SessionStore` implements both `SessionStorage` and `EventSink`
- `SessionStore` gains a `Mutex<Vec<Event>>` buffer field; `flush()` writes atomically
- `SessionMeta` and `render_session()` unchanged
- Adds `InMemorySessionStorage` behind `test-support`

**braid-mcp:**
- No changes — `McpToolRegistry` uses a closure executor, deps remain `braid-model` only

## Composition Root (`braid-cli`)

```rust
// cmd_run() — explicit wiring, no business logic
let redactor = RedactionPipeline::new()
    .with_rule(SecretPatternRule::new())
    .with_rule(EnvVarRule::new())
    .with_rule(HomePathRule::new());

// Arc allows the store to be shared: Engine holds it as EventSink,
// cmd_sessions holds it as SessionStorage — same instance, no double-open.
let store = Arc::new(SessionStore::open(default_store_dir()?)?);

let hooks = HookRegistry::fail_closed()
    .register(DestructiveCommandGuard::new());

let tools = HookedExecutor::new(ToolRegistry::new(), hooks, session_id.clone());

let provider = resolve_provider(provider_flag, model)?;

let engine = Engine::new(provider, tools, Arc::clone(&store), redactor);
let output = engine.run(RunInput { session_id, messages, max_turns }, &SimpleLoopPlanner)?;
```

`SessionStore` implements `EventSink` and `SessionStorage` for `Arc<SessionStore>` as well as `SessionStore`, so the cloned `Arc` satisfies both bounds. `cmd_sessions` operations (list, load, prune) use the same `store` handle — no second filesystem open needed.

Swapping `OpenAiProvider` for `MockProvider` or `SessionStore` for `InMemorySessionStorage` is a one-line change at the composition root.

## Error Handling

All port trait methods return `Result<_>`. Adapters map infrastructure errors to domain errors at the boundary — no infrastructure error types leak through port signatures.

## Testing Strategy

- **Domain/engine unit tests:** Use `MockProvider` + `VecEventSink` + `PassthroughRedactor` + `StaticTool`. No I/O, no network.
- **Adapter tests:** Each adapter tested independently against its port contract. Filesystem tests in `braid-observe`, HTTP tests in `braid-providers` (behind an integration feature).
- **CLI integration tests:** Wire real adapters in a temp dir, invoke `cmd_run`. Already scaffolded in `crates/braid-cli/tests/`.

## What Does Not Change

- `braid-model` — unchanged, still the leaf
- `braid-mcp` — unchanged, deps stay at `braid-model` only
- The `Planner` trait and `SimpleLoopPlanner` — stay in `braid-core`
- `ToolRegistry` — stays in `braid-core` as a utility
- `RedactionRule` — stays in `braid-redact` as an internal building block
- The four-phase build order and overall crate naming

## Migration Order

1. Create `braid-ports` with all trait definitions and supporting types (`HookContext`, `HookVerdict`). No implementations. Add to workspace `Cargo.toml`.
2. Update `braid-core`: replace local `Provider`/`ToolExecutor` with `pub use braid_ports::{Provider, ToolExecutor}` (not re-declared); add `EventSink` and `Redactor` imports; update `Engine` to `Engine<P, T, S, R>`; drop `events` from `RunOutput`; call `event_sink.flush()` at end of `run()`; update `Engine::new()` to `(provider, tool_executor, event_sink, redactor)` order; update all existing engine unit tests.
3. Update `braid-providers`: import `Provider` from `braid-ports`; drop `braid-core` dep; add `MockProvider` behind `test-support`.
4. Update `braid-hooks`: import `Hook`, `HookContext`, `HookVerdict`, `ToolExecutor` from `braid-ports`; drop `braid-core` dep.
5. Update `braid-redact`: add `braid-ports` dep; implement `Redactor` on `RedactionPipeline`; keep `braid-model` dep.
6. Update `braid-observe`: add `braid-ports` dep; implement `SessionStorage` and `EventSink` on `SessionStore` (with `Mutex<Vec<Event>>` buffer); add `InMemorySessionStorage` behind `test-support`.
7. Update `braid-cli`: wire new `Engine<P, T, S, R>` signature; update `cmd_sessions` to use `SessionStorage` trait where possible.
8. Update all `Cargo.toml` files to reflect the new dep graph.
