# braid-ports

`braid-ports` defines the hexagonal interfaces between Braid's domain engine and its adapters. The
crate contains traits and small contract types, but no concrete provider, persistence, redaction,
hook, context, or component implementation.

## Workspace role

The crate sits between `braid-model` and all behavior-bearing crates. `braid-engine` consumes these
ports, while provider, observability, redaction, hook, context, and component crates implement them.
This dependency direction keeps the engine independent of infrastructure choices.

## Port traits

| Trait | Contract |
| --- | --- |
| `Provider` | Complete a `ProviderRequest` and return a `ProviderResponse` |
| `ToolExecutor` | Execute one `ToolCall` and return a `ToolResult` |
| `Redactor` | Return a redacted copy of a `Message` |
| `EventSink` | Record events and optionally flush buffered state |
| `SessionStorage` | Write, load, list, and prune session events |
| `ContextProvider` | Assemble or refresh a `ContextSnapshot` |
| `Hook` | Evaluate before execution and observe successful results afterward |
| `ComponentRegistry` | Load, query, and list component metadata |

`Provider`, `ToolExecutor`, and `Redactor` have forwarding implementations for boxed or shared
trait objects where needed. `EventSink` supports both `Box<T>` and `Arc<T>`, and defaults `flush()`
to a no-op. `SessionStorage` and `ContextProvider` support shared `Arc<T>` implementations.

## Hook and component types

- `HookContext` carries the `SessionId` and `ToolCall` being evaluated.
- `HookVerdict` is either `Allow` or `Deny { reason, remediation }`.
- `ComponentInfo` exposes a component's name, version, and description.

## Usage

```rust
use anyhow::Result;
use braid_model::{ToolCall, ToolResult};
use braid_ports::ToolExecutor;

struct Echo;

impl ToolExecutor for Echo {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        Ok(ToolResult {
            name: call.name,
            output: call.input,
        })
    }
}
```

## Implementation status

All declared ports are active contracts with concrete implementations elsewhere in the workspace.
The traits are synchronous; asynchronous transport concerns, such as MCP stdio, stay in adapters.

## Features

This crate has no Cargo feature flags.

## Development and testing

Changes to a port can affect every adapter and should be checked across the workspace:

```console
cargo check -p braid-ports
cargo nextest run --workspace
cargo run -p xtask -- check-deps
```
