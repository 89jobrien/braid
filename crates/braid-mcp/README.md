# braid-mcp

`braid-mcp` exposes Braid tool definitions over a small Model Context Protocol stdio server. It uses
JSON-RPC 2.0 messages framed with `Content-Length` headers.

## Workspace role

This is a transport adapter. It maps MCP requests to `braid-model::ToolCall` values through a
registry-provided executor closure and maps `ToolResult` values back to MCP text content. It does
not own orchestration or provider logic.

## Protocol support

The server advertises protocol version `2024-11-05` and handles:

| Method | Behavior |
| --- | --- |
| `initialize` | Return tool capability and server name/version |
| `notifications/initialized` | Consume the notification without writing a response |
| `tools/list` | Return registered definitions as MCP input schemas |
| `tools/call` | Validate the tool name and invoke the executor |

Malformed JSON receives error `-32700`, unknown methods receive `-32601`, and failed or unknown tool
calls receive `-32602`. Frames without a valid `Content-Length` header are skipped.

## Key APIs and usage

```rust
use braid_mcp::{McpToolRegistry, echo_tool, run_mcp_server};

let registry = McpToolRegistry::new(|call| {
    Ok(braid_model::ToolResult {
        name: call.name,
        output: call.input,
    })
})
.register(echo_tool());

tokio::runtime::Runtime::new()?.block_on(run_mcp_server(registry))?;
# Ok::<(), anyhow::Error>(())
```

`McpToolRegistry::list_tools` exposes registered schemas. `call_tool` rejects names that were not
registered before constructing an ID of the form `mcp_<name>` and serializing arguments into the
tool call's string input.

The built-in `echo_tool()` defines one required string property named `message`. The CLI wires a
matching executor that returns that field.

## Implementation status

The stdio server, registry, initialization, listing, calls, framing, and echo definition are
implemented. There is no HTTP transport, resource/prompt capability, streaming result support,
client implementation, or cancellation handling. The public server owns process stdin/stdout.

## Features

This crate has no Cargo feature flags.

## Development and testing

```console
cargo nextest run -p braid-mcp
cargo clippy -p braid-mcp -- -D warnings
cargo check -p braid-mcp
cargo run -p braid-cli -- mcp
```

Unit tests call the request handler directly for initialization, listing, successful calls, unknown
methods, and unknown tools.
