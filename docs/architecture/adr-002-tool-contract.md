# ADR-002: Tool Contract

**Date:** 2026-04-01
**Status:** Accepted

## Context

Tools are the primary extension point for Braid sessions. The contract between the engine, hooks, and tool implementations must be stable and testable without requiring a live provider or real side effects.

## Decision

The tool contract is defined by three traits in `braid-ports` and `braid-core`:

```rust
// braid-ports — inner-ring port
pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
    fn list_tools(&self) -> Vec<ToolDefinition>;
}

// braid-core — registry delegates to registered Tool impls
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value>;
}
```

Key invariants:

- **Input and output are `serde_json::Value`.** Tools do not depend on provider-specific types. The engine serializes/deserializes at the boundary.
- **Tools are registered, not hardcoded.** `ToolRegistry` holds `Box<dyn Tool>` entries; the engine queries `list_tools()` to populate the provider request.
- **Hooks wrap the executor, not the tool.** `HookedExecutor<T: ToolExecutor>` intercepts calls at the executor boundary. Individual `Tool` implementations are unaware of hook policy.
- **Fail-closed by default.** If a pre-execution hook returns `Deny`, the call is rejected before the tool runs. Hook errors (not just denials) also reject by default.
- **`ToolCall` and `ToolResult` are defined in `braid-model`.** No other crate invents parallel tool I/O types.

## Consequences

- Any `ToolExecutor` implementation (real, mock, hooked) is substitutable in tests and production.
- Adding a new tool requires implementing `Tool` and registering it — no engine changes.
- Hook policy is composable: wrap any executor with `HookedExecutor` to apply guards without touching tool code.
- MCP exposure (`braid-mcp`) uses the same `ToolRegistry`; tools registered once are callable both in-process and via MCP.
