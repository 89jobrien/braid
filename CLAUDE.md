# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
just              # Default: cargo check
just fmt          # Format: cargo fmt --all
just check        # Check workspace: cargo check --workspace
just test         # Run tests: cargo test --workspace

cargo run -p braid-cli                       # Run the CLI
cargo test -p braid-core                     # Test a single crate
cargo test -p braid-core -- test_name        # Run a single test by name
cargo clippy --workspace                     # Lint
```

## Rust Toolchain

Edition **2024**, minimum rust-version **1.88**, stable channel. Clippy and rustfmt are required components (see `rust-toolchain.toml`).

## Architecture

Braid is a **personal agent platform** built as a Rust workspace. It consolidates ideas from 20+ older projects, treating them as design donors — not merge targets. Build from scratch; never mechanically port old code.

### Four-Phase Build Order

**Phase 1 (active)**: Four foundational crates forming a minimal runnable vertical slice:

| Crate | Role |
|---|---|
| `braid-model` | Canonical domain types — single source of truth. No other crate invents parallel domain models. |
| `braid-core` | Runtime engine (`Engine<T, P>`), `Provider` trait, `ToolExecutor` trait. No provider logic lives here. |
| `braid-providers` | Provider adapters (currently `MockProvider`). Real adapters go here, not in core or CLI. |
| `braid-cli` | Thin operator entrypoint. Delegates to core, not vice versa. |

**Crate dependency graph**: `braid-cli → braid-providers → braid-core → braid-model`. Model is the leaf; CLI is the root. All domain types live in `braid-model`; all traits (`Provider`, `ToolExecutor`) live in `braid-core`.

**Phases 2–4 (planned, not started)**: `braid-hooks`, `braid-mcp`, `braid-redact`, `braid-observe`, `braid-context`, `braid-bootstrap`, `braid-components`.

### Data Flow

```
CLI → Engine::run(RunInput)
        ├─ Provider::complete(ProviderRequest) → ProviderResponse
        ├─ ToolExecutor::execute(ToolCall) → ToolResult
        └─ Emits 4 Events: SessionStarted, ProviderResponded, ToolCalled, ToolCompleted
      → RunOutput { provider_response, tool_result, events }
```

### Hard Boundaries (from design docs)

- Redaction (`braid-redact`) ≠ observability (`braid-observe`)
- MCP (`braid-mcp`) ≠ orchestration (that's `braid-core`)
- CLI is a thin front door; session control belongs to core
- Hooks are external policy, not baked into the engine loop
- **Redact-before-persist**: privacy by default

### Workspace Dependencies

All crates share: `anyhow`, `serde` (with derive), `thiserror`, `tracing` (declared, not yet used).

## Design Principles

- **Minimal vertical slice first**: Complete Phase 1 fully before adding hooks, MCP, or observability.
- **No cargo-cult porting**: Only rebuild subsystems that serve the final platform shape.
- **Bounded context**: Extractors should be selective, not endless ingestion frameworks.

## Planning Docs

Comprehensive design documents live in `docs/planning/`:
- `Braid - Rust Workspace Spec.md` — most detailed architecture spec
- `Braid - Crate Implementation Checklist.md` — phase-by-phase deliverables
- `Braid - Rust Workspace Blueprint.md` — rationale for Rust-first, scratch-build decision
