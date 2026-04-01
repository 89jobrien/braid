# Braid

Rust-first workspace for the Braid personal agent platform.

## Crates

**Phase 1 — Core**
- `braid-model`: canonical domain types (messages, tool calls, transcripts, context)
- `braid-ports`: inner-ring port traits (`Provider`, `ToolExecutor`, `EventSink`, `SessionStorage`, `ContextProvider`)
- `braid-core`: runtime engine (`Engine<P, T, S, R, C>`), `SimpleLoopPlanner`, tool registry
- `braid-providers`: provider adapters (`MockProvider`, `OpenAiProvider`)
- `braid-cli`: thin operator entrypoint for local runs

**Phase 2 — Safety**
- `braid-redact`: `RedactionPipeline` with ordered rule chain (secrets, env vars, home paths)
- `braid-hooks`: `HookedExecutor` wrapping any `ToolExecutor` with pre/post hook gating
- `braid-mcp`: MCP server over stdio (JSON-RPC), tool registration and dispatch
- `braid-observe`: session event store; persists `Event` stream to disk

**Phase 3 — Context**
- `braid-context`: context assembly from `DoobSource` (todos) and `RepoSource` (git diff/log); two-stage compaction (staleness filter + token budget with rolling LLM summarization); injected into engine at session start via `ContextProvider` port

**UI**
- `braid-tui`: multi-pane session inspector (Ratatui)

## Docs

- [Planning Docs](./docs/planning/README.md)
- [Workspace Overview](./docs/architecture/workspace-overview.md)
- [Braid](./docs/planning/Braid.md)
- [Braid - Rust Workspace Blueprint](./docs/planning/Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Braid - Rust Workspace Spec](./docs/planning/Braid%20-%20Rust%20Workspace%20Spec.md)
- [Braid - Crate Implementation Checklist](./docs/planning/Braid%20-%20Crate%20Implementation%20Checklist.md)
