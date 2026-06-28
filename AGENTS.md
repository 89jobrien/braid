# Braid — Agent Operating Guide

Braid is a personal agent platform built as a Rust workspace (edition 2024). It
consolidates ideas from 20+ older projects into a minimal runnable agent engine with
runtime safety layers, MCP integration, and observability. Work always happens on
feature branches; never commit directly to main.

## Primary Toolkit: `just` and `cargo nextest`

All development workflows use standard Rust commands. The `justfile` wraps common patterns
for quick access. Prefer `cargo nextest run` over `cargo test` for parallel filtering.

### Build & Quality

| Command                                  | Purpose                                 |
| ---------------------------------------- | --------------------------------------- |
| `just fmt`                               | Format all Rust code (cargo fmt)        |
| `just check`                             | Check workspace (cargo check)           |
| `just clippy`                            | Lint: cargo clippy --workspace          |
| `just test`                              | Run tests: cargo nextest run --workspace|
| `just pre-commit`                        | Full gate: fmt + build + clippy + test  |
| `cargo nextest run --workspace`          | Tests (preferred over cargo test)       |
| `cargo nextest run -p braid-core`        | Test single crate                       |
| `cargo nextest run -- test_name`         | Filter tests by name                    |
| `cargo build --release --workspace`      | Release build (pre-commit step)         |

### CI / Workflows

`.github/workflows/ci.yml` — fmt → clippy → test (nextest), plus independent
`audit` (cargo-deny + cargo-audit) and `lint` (cargo-machete) jobs.

`.github/workflows/nightly.yml` — cargo-geiger unsafe audit at 2am UTC.

`deny.toml` — license/ban/advisory policy (mirrors minibox).

### Unsafe Code Policy

`unsafe_code = "deny"` workspace-wide. Test modules using `set_var`/`remove_var`
(Rust 2024 requires `unsafe {}`) must add `#[allow(unsafe_code)]` to the `mod tests`
block, not individual functions or the whole crate.

## Workspace Layout

```
braid/
├── crates/
│   ├── braid-model/      # Canonical domain types (leaf crate)
│   ├── braid-ports/      # Hexagonal ports and traits
│   ├── braid-core/       # Runtime engine, Provider/Planner/ToolExecutor traits
│   ├── braid-providers/  # Provider adapters (MockProvider, OpenAiProvider)
│   ├── braid-cli/        # Thin operator entrypoint
│   ├── braid-redact/     # RedactionPipeline, SecretPatternRule, EnvVarRule
│   ├── braid-hooks/      # Hook trait, HookedExecutor, DestructiveCommandGuard
│   ├── braid-mcp/        # MCP server (stdio JSON-RPC, tokio async)
│   ├── braid-observe/    # Event sink and session storage (Phase 3)
│   ├── braid-tui/        # TUI framework (planned, Phase 3)
│   ├── braid-context/    # Context injection (planned, Phase 3)
│   └── braid-bootstrap/  # Session bootstrap (planned, Phase 4)
├── xtask/                # Task runner (cargo xtask, minimal)
├── justfile              # Command aliases for common tasks
├── deny.toml             # License/ban/advisory policy
└── Cargo.toml            # Workspace config
```

**Crate dependency graph** (always respect this order): `braid-cli →
braid-providers → braid-core → {braid-ports, braid-model}`. Model is the leaf; CLI is
the root. All domain types live in `braid-model`; all traits (`Provider`, `Planner`,
`ToolExecutor`, `Hook`) live in `braid-core` or `braid-ports`.

## Core Architecture

### Four-Phase Build Order

**Phase 1 (complete)** — Minimal vertical slice with foundational crates:

- `braid-model`: Single source of truth for domain types
- `braid-core`: Runtime engine (`Engine<T, P>`), trait definitions
- `braid-providers`: Provider adapters (OpenAI, mock)
- `braid-cli`: Thin operator frontend

**Phase 2 (complete)** — Safety and tool exposure:

- `braid-redact`: Ordered redaction rule chain (secrets, env vars, paths)
- `braid-hooks`: Pre/post hook gating with `HookedExecutor<T>`
- `braid-mcp`: MCP server over stdio, tool registry, async (tokio)

**Phase 3 (planned)** — Observability and context:

- `braid-observe`: Event sink, session storage, state reconciliation
- `braid-tui`: TUI framework for interactive control
- `braid-context`: Context injection and session bootstrapping

**Phase 4 (planned)** — Bootstrap and deployment:

- `braid-bootstrap`: Session initialization, middleware orchestration

### Runtime Data Flow

```
CLI → Engine::run(RunInput { session_id, messages, max_turns }, &SimpleLoopPlanner)
        Loop (Planner::next_action → Action):
          ├─ CallProvider  → Provider::complete() → extract tool calls
          ├─ ExecuteTool   → ToolExecutor::execute() → tool result as message
          └─ Finish        → return RunOutput
        Emits: SessionStarted, ProviderResponded, ToolCalled, ToolCompleted
      → RunOutput { provider_response, events }
```

**Core message types** in `braid-model`: `Message` (role + content parts), `ContentPart`
(Text/Image/ToolUse/ToolResult), `Role` (System/User/Assistant/Tool), `ToolCall`
(id/name/input), `Transcript`, `SessionPhase`.

**Hard boundaries:**
- Redaction (`braid-redact`) ≠ observability (`braid-observe`)
- MCP (`braid-mcp`) ≠ orchestration (core domain)
- CLI delegates to core; core never references CLI
- Hooks are external policy, not baked into engine
- **Redact before persist** — privacy by default

## Code Conventions

### Rust (Edition 2024)

- **Line width**: 100 characters
- **Minimum rust version**: 1.88 (pinned in `rust-toolchain.toml`)
- **Error handling**: `anyhow::Result<T>`, propagate with `?`
- **No unwrap() in production code** — only in tests/examples
- **Naming**: PascalCase structs/enums, snake_case functions, SCREAMING_SNAKE_CASE constants
- **Imports**: Group by external crates, then std
- **Tests**: Unit tests in `mod tests {}`, integration in `tests/`
- **Linting**: `cargo clippy --workspace -- -D warnings` (enforced in CI)

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`

## Workflow Guides

### Implement a Feature

1. Create feature branch: `git checkout -b feat/description`
2. Implement changes (respect hexagonal architecture)
3. Validate: `just pre-commit` (fmt + build + clippy + test)
4. Commit: `git commit -m "feat(scope): description"`
5. Push to github remote: `git push github feat/description`
6. Open PR: `gh pr create --title "feat(scope): description" --body "..."`

### Debug a CI Failure

1. Identify which job failed: `gh run view <run-id>`
2. Mirror locally: `just pre-commit` or `just clippy`, `just test`
3. Fix the issue and retry: `git push` (CI re-runs automatically)

### Rebase Conflicts on Phase 3a Changes

When rebasing a branch predating Phase 3a onto current main, `braid-observe/src/store.rs`
may need manual merge:

1. Take `--theirs` for conflicted file
2. Add whichever half is missing: hexagonal refactor OR Phase 3a code
3. Test: `cargo check && cargo test`

### Release & Versioning

Current version: `0.1.0` (workspace.package in Cargo.toml). Follow semver on next
release. No automated release workflow yet.

## Session Preflight Checking

When adding session-gating preconditions (provider availability, credential validity,
network access), evaluate whether it belongs in `Engine::preflight()` (before run loop)
or inline validation (on request path). Preflight checks probed external state
**before startup**; inline validation handles **request-shape errors**.

## Git Workflow

- All automated and council-driven work happens on **feature branches**
- Branch naming: `feat/description`, `council/YYYY-MM-DD-description`
- Push to `github` remote (primary mirror), `origin` (gitea self-hosted)
- **Never commit directly to main**
- Squash-merged branches: use `git branch -D` (not `-d`, which fails on squash)
- Remove worktrees before deleting branches: `git worktree remove <path>`

## Key Dependencies

- **Core**: `anyhow`, `serde`, `serde_json`, `thiserror`, `tracing`
- **Providers**: `reqwest` (blocking HTTP)
- **Redaction**: `regex`
- **MCP**: `tokio` (async runtime)
- **OpenSSL**: `openssl { features = ["vendored"] }` (cross-compile support)

## Environment Variables

| Variable                    | Purpose                                          |
| --------------------------- | ------------------------------------------------ |
| `RUST_LOG`                  | Tracing level (debug, info, warn, error)        |
| `RUST_BACKTRACE`            | Backtrace on panic (1 = short, full = complete) |
| `BRAID_MODEL_TEMPERATURE`   | LLM temperature (0.0–2.0, default from provider)|
| `BRAID_MODEL_MAX_TOKENS`    | Max output tokens (default from provider)       |

## Testing & Isolation

- Use dependency injection for paths (no hardcoded `/tmp` or `/var`)
- Pass `TempDir` to functions that read/write files
- No `std::env::set_var` without restoration (use guard pattern)
- Test isolation: parallel tests via `cargo nextest` (no `serial_test`)

## Pre-Commit Hooks

Run `just install-hooks` once after cloning to set up git hooks that enforce:

- `cargo fmt --check`
- `cargo build --release` (entire workspace)
- `cargo clippy --workspace`
- `cargo nextest run --workspace`

Bypass in emergencies with `--no-verify` (not recommended).

## Documentation & Specs

Implementation specs and plans live in `docs/superpowers/specs/` and
`docs/superpowers/plans/`.

Comprehensive design documents in `docs/planning/`:

- `Braid - Rust Workspace Spec.md` — detailed architecture
- `Braid - Crate Implementation Checklist.md` — phase-by-phase deliverables
- `Braid - Rust Workspace Blueprint.md` — rationale for Rust-first approach

When writing a spec under `docs/`, add a HANDOFF tracking item
`"Implement: <spec-name>"` with `status: open`.
