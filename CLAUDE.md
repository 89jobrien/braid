# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
just              # Default: cargo check
just fmt          # Format: cargo fmt --all
just check        # Check workspace: cargo check --workspace
just clippy       # Lint: cargo clippy --workspace -- -D warnings
just test         # Run tests: cargo nextest run --workspace

cargo run -p braid-cli                       # Run the CLI
cargo test -p braid-core                     # Test a single crate
cargo test -p braid-core -- test_name        # Run a single test by name
cargo clippy --workspace                     # Lint
```

## CI / Workflows

`.github/workflows/ci.yml` — fmt → clippy → test (nextest), plus independent `audit` (cargo-deny + cargo-audit) and `lint` (cargo-machete) jobs.
`.github/workflows/nightly.yml` — cargo-geiger unsafe audit at 2am UTC.
`deny.toml` — license/ban/advisory policy (mirrors minibox).

## Rust Toolchain

Edition **2024**, minimum rust-version **1.88**, stable channel. Clippy and rustfmt are required components (see `rust-toolchain.toml`).

## Architecture

Braid is a **personal agent platform** built as a Rust workspace. It consolidates ideas from 20+ older projects, treating them as design donors — not merge targets. Build from scratch; never mechanically port old code.

### Four-Phase Build Order

**Phase 1 (complete)**: Four foundational crates forming a minimal runnable vertical slice:

| Crate | Role |
|---|---|
| `braid-model` | Canonical domain types — single source of truth. No other crate invents parallel domain models. |
| `braid-core` | Runtime engine (`Engine<T, P>`), `Provider`/`Planner`/`ToolExecutor` traits, `SimpleLoopPlanner`. No provider logic lives here. |
| `braid-providers` | Provider adapters (`MockProvider`, `OpenAiProvider`). Real adapters go here, not in core or CLI. |
| `braid-cli` | Thin operator entrypoint. Delegates to core, not vice versa. |

**Crate dependency graph**: `braid-cli → braid-providers → braid-core → braid-model`. Model is the leaf; CLI is the root. All domain types live in `braid-model`; all traits (`Provider`, `ToolExecutor`) live in `braid-core`.

**Phase 2 (complete)**: Safety and tool-exposure layer:

| Crate | Role |
|---|---|
| `braid-redact` | `RedactionPipeline` with ordered `RedactionRule` chain. Built-in rules: `SecretPatternRule`, `EnvVarRule`, `HomePathRule`. Walks `Message`/`Event` types. |
| `braid-hooks` | `Hook` trait with `HookVerdict` (Allow/Deny). `HookedExecutor<T: ToolExecutor>` wraps any executor with pre/post hook gating. Built-in: `DestructiveCommandGuard`, `FreshnessGuard` (placeholder). Engine/Planner unchanged. |
| `braid-mcp` | MCP server over stdio (JSON-RPC). `McpToolRegistry` for tool registration/dispatch. Echo tool. Only async crate (tokio). CLI `mcp` subcommand. |

**Phases 3–4 (planned, not started)**: `braid-observe`, `braid-context`, `braid-bootstrap`, `braid-components`.

### Data Flow

```
CLI → Engine::run(RunInput { session_id, messages, max_turns }, &SimpleLoopPlanner)
        Loop (driven by Planner::next_action → Action):
          ├─ CallProvider  → Provider::complete(ProviderRequest) → extract tool calls
          ├─ ExecuteTool   → ToolExecutor::execute(ToolCall) → tool result as message
          └─ Finish        → return RunOutput
        Emits: SessionStarted, ProviderResponded, ToolCalled, ToolCompleted, SessionCompleted
      → RunOutput { provider_response, events }
```

### Core Message Types

`braid-model` defines the conversation model: `Message` (role + content parts), `ContentPart` (Text/Image/ToolUse/ToolResult), `Role` (System/User/Assistant/Tool), `ToolCall` (id/name/input), `TokenCount`, `Transcript`, `SessionPhase`. Provider types use `Message` — not plain strings.

### Hard Boundaries (from design docs)

- Redaction (`braid-redact`) ≠ observability (`braid-observe`)
- MCP (`braid-mcp`) ≠ orchestration (that's `braid-core`)
- CLI is a thin front door; session control belongs to core
- Hooks are external policy, not baked into the engine loop
- **Redact-before-persist**: privacy by default

### Workspace Dependencies

All crates share: `anyhow`, `serde` (with derive), `serde_json`, `thiserror`, `tracing` (declared, not yet used). `braid-providers` also uses `reqwest` (blocking). `braid-redact` uses `regex`. `braid-mcp` uses `tokio`.

## Git Workflow

- All automated and council-driven work must be done on a **feature branch**, not committed directly to `main`.
- Branch naming: `council/YYYY-MM-DD-description` for council sessions, `feat/description` for features.
- Push feature branches to the `github` remote and open a PR targeting `main`.
- The `gitea` remote (`origin`) is the self-hosted primary; `github` is the GitHub mirror.

## Design Principles

- **Minimal vertical slice first**: Complete each phase fully before starting the next.
- **No cargo-cult porting**: Only rebuild subsystems that serve the final platform shape.
- **Bounded context**: Extractors should be selective, not endless ingestion frameworks.

## Planning Docs

Implementation specs and plans live in `docs/superpowers/specs/` and `docs/superpowers/plans/`.

Comprehensive design documents live in `docs/planning/`:
- `Braid - Rust Workspace Spec.md` — most detailed architecture spec
- `Braid - Crate Implementation Checklist.md` — phase-by-phase deliverables
- `Braid - Rust Workspace Blueprint.md` — rationale for Rust-first, scratch-build decision
