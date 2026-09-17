# Braid

Rust-first workspace for the Braid personal agent platform.

## Crates

**Core runtime**

- `braid-model`: canonical domain types (messages, tool calls, transcripts, context)
- `braid-ports`: inner-ring port traits (`Provider`, `ToolExecutor`, `EventSink`, `SessionStorage`, `ContextProvider`)
- `braid-engine`: runtime engine (`Engine<P, T, S, R, C>`), `SimpleLoopPlanner`, tool registry
- `braid-providers`: provider adapters (`MockProvider`, `OpenAiProvider`)

**Safety and integration**

- `braid-redact`: `RedactionPipeline` with ordered rule chain (secrets, env vars, home paths)
- `braid-hooks`: `HookedExecutor` wrapping any `ToolExecutor` with pre/post hook gating
- `braid-mcp`: MCP server over stdio (JSON-RPC), tool registration and dispatch

**State and context**

- `braid-observe`: session event storage, ingest, replay, and terminal rendering
- `braid-context`: context assembly from `DoobSource` (todos) and `RepoSource` (git diff/log); two-stage compaction (staleness filter + token budget with rolling LLM summarization); injected into engine at session start via `ContextProvider` port
- `braid-components`: manifest-backed command and prompt component registry
- `braid-bootstrap`: setup, configuration, and environment health checks

**Operator interfaces**

- `braid-cli`: command-line entrypoint for runs, agents, setup, sessions, MCP, and components
- `braid-tui`: multi-pane session inspector (Ratatui)

## Docs

- [Roadmap](./ROADMAP.md)
- [Planning Docs](./docs/planning/README.md)
- [Workspace Overview](./docs/architecture/workspace-overview.md)
- [Braid](./docs/planning/Braid.md)
- [Braid - Rust Workspace Blueprint](./docs/planning/Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Braid - Rust Workspace Spec](./docs/planning/Braid%20-%20Rust%20Workspace%20Spec.md)
- [Braid - Crate Implementation Checklist](./docs/planning/Braid%20-%20Crate%20Implementation%20Checklist.md)
