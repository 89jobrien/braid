# ADR-002: Tool Contract

**Status:** Accepted
**Date:** 2026-07-13
**Deciders:** Joseph O'Brien

## Context

The engine loop needs to invoke tools (shell commands, MCP handlers, etc.) and
apply policy (destructive-command blocking, freshness checks) without coupling
the engine to any specific tool implementation or policy set. The contract must
be composable and testable without a live execution environment.

## Decision

**Port (`braid-ports`)** defines the minimal trait surface:

```rust
pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}
```

`ToolCall` carries `id`, `name`, and `input` (raw string). `ToolResult` carries
`name` and `output` (raw string). Executors return `anyhow::Result<ToolResult>`;
there is no typed error hierarchy for tool failures.

**Hook layer (`braid-hooks`)** wraps any executor via `HookedExecutor<T>`:

```rust
pub struct HookedExecutor<T: ToolExecutor> {
    inner: T,
    registry: HookRegistry,
    session_id: SessionId,
}
```

Before each call, `HookRegistry::evaluate` runs all registered `Hook`
implementations. The verdict is binary — `HookVerdict::Allow` or
`HookVerdict::Deny { reason, remediation }`. A `Deny` short-circuits execution
and converts to an `anyhow` error; the inner executor is never called. After a
successful call, `registry.notify_post` broadcasts the result to all hooks for
observability (no verdict on post).

`Hook` is defined in `braid-ports`:

```rust
pub trait Hook: Send + Sync {
    fn name(&self) -> &'static str;
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    fn post_execute(&self, _ctx: &HookContext, _result: &ToolResult) {}
}
```

Built-in hook: `DestructiveCommandGuard` (pattern-matches `rm -rf` and similar).

## Consequences

- **Composable**: any `ToolExecutor` gains hook gating by wrapping — engine and
  planner are unchanged.
- **String output**: callers receive raw text; structured parsing is the
  executor's responsibility, not the trait's.
- **Binary verdict**: hooks cannot partially modify a call — only allow or deny.
  This prevents hooks from becoming a transformation pipeline (by design).
- **No typed errors**: `anyhow::Result` keeps the trait simple; callers cannot
  pattern-match on tool-failure variants without downcasting.

## Alternatives Considered

- **Typed error enum**: would require all executor implementations to map errors
  into a shared type; `anyhow` keeps integration cost low at the expense of
  inspectability.
- **Middleware chain (onion model)**: hooks as nested wrappers rather than a
  registry. Rejected — ordered hooks in a registry are easier to reason about
  and test independently.
- **Post-hook verdict**: allowing post-hooks to deny or retry. Rejected —
  post-execution side effects are simpler to reason about when they cannot alter
  the outcome.
