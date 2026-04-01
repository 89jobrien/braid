---
type: design
topic: [braid, bootstrap, phase4]
status: approved
tags: [braid, bootstrap, doctor, setup, hexagonal]
---

# braid-bootstrap Design

**Date:** 2026-04-01
**Phase:** 4 — Operator Layer
**Status:** Approved

## Summary

`braid-bootstrap` is a library crate that provides structured environment health checks (`doctor`) and a non-interactive local setup flow (`setup`). `braid-cli` delegates both the `doctor` and `setup` subcommands to it. No other crate depends on `braid-bootstrap`.

## Architecture

### Crate boundary

`braid-bootstrap` is a pure library crate with no dependency on any other braid crate. It depends only on workspace-level deps (`anyhow`, `serde`, `serde_json`) and `reqwest` (blocking) for the OpenAI connectivity check — calling the API directly rather than importing `braid-providers` to avoid pulling in `braid-core`.

Dependency addition: `braid-cli` gains `braid-bootstrap = { path = "../braid-bootstrap" }`.

### Core types

```rust
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,      // human-readable detail + remediation hint on Fail
}

pub trait Check: Send + Sync {
    fn run(&self) -> CheckResult;
}
```

`Check::run()` never returns `Err` — all failures are represented as `CheckStatus::Fail` with a descriptive message.

### Checks

**Existing (extracted from `braid-cli::mod doctor`):**
- `RustToolchainCheck` — verifies `rustc >= 1.88`
- `OpenAiKeyCheck` — checks `OPENAI_API_KEY` env var is set
- `OllamaConnectivityCheck` — HTTP probe to `http://localhost:11434/api/tags`
- `OpenAiConnectivityCheck` — sends a minimal request via `OpenAiProvider`; skipped (Warn) if no key
- `WorkspaceHealthCheck` — runs `cargo check --workspace`

**New:**
- `GitCheck` — `git --version` is present and runnable
- `DoobCheck` — `doob --version` is present; Warn (not Fail) if missing
- `CargoNextestCheck` — `cargo nextest --version` is present
- `CargoDenyCheck` — `cargo deny --version` is present
- `BraidConfigDirCheck` — `~/.braid/` exists; Warn if missing (setup can fix it)

### Renderer

```rust
pub struct TerminalRenderer;

impl TerminalRenderer {
    pub fn render(results: &[CheckResult]);        // colorized ANSI output
    pub fn render_plain(results: &[CheckResult]) -> String;  // stripped, for tests
}
```

ANSI codes hand-rolled (`\x1b[32m` green / `\x1b[33m` yellow / `\x1b[31m` red). No new deps.

Output format per check:
```
git                    ... ok (git version 2.44.0)
doob                   ... warn (not found — install with: cargo install doob)
OPENAI_API_KEY         ... ok
openai connectivity    ... ok
```

### Setup flow

`braid_bootstrap::setup::run() -> Result<()>`

Non-interactive, idempotent. Creates:
- `~/.braid/` directory
- `~/.braid/config.toml` with default content (does not overwrite if exists)

Default `config.toml`:
```toml
[provider]
default = "openai"
model   = "gpt-4o"

[session]
max_turns = 20

[context]
budget_tokens = 2000
```

Prints each action taken:
```
created  ~/.braid/
created  ~/.braid/config.toml
skipped  ~/.braid/config.toml (already exists)
```

Returns `Err` only on filesystem errors (e.g., permission denied creating the directory).

### `braid-cli` changes

- Remove `mod doctor` inline module from `main.rs`
- `cmd_doctor()` → calls `braid_bootstrap::doctor::run_checks()` + `TerminalRenderer::render()`
- Add `Command::Setup` subcommand → calls `braid_bootstrap::setup::run()`
- `braid-bootstrap` added to `braid-cli/Cargo.toml`

## Module layout

```
crates/braid-bootstrap/
  src/
    lib.rs          — re-exports
    checks/
      mod.rs        — Check trait, CheckResult, CheckStatus
      toolchain.rs  — RustToolchainCheck
      keys.rs       — OpenAiKeyCheck
      connectivity.rs — OllamaConnectivityCheck, OpenAiConnectivityCheck
      workspace.rs  — WorkspaceHealthCheck
      tools.rs      — GitCheck, DoobCheck, CargoNextestCheck, CargoDenyCheck
      config.rs     — BraidConfigDirCheck
    doctor.rs       — run_checks() → Vec<CheckResult>
    render.rs       — TerminalRenderer
    setup.rs        — run() -> Result<()>
```

## Error Handling

- `Check::run()` → always `CheckResult`, never panics
- `setup::run()` → `Result<()>`, fails fast on fs errors
- Connectivity checks that require external processes use the same 5s timeout pattern as `braid-context` sources

## Testing

- Unit test each `Check` impl: mock the subprocess or env var, verify `CheckStatus` and message content
- `TerminalRenderer::render_plain()` tested with known input/output
- `setup::run()` tested against `tempfile::tempdir()` — verify files created, verify idempotent (second run does not overwrite)
- No snapshot tests — output format is simple enough for `assert!(output.contains(...))`

## Non-Goals

- No interactive prompts
- No package manager integration (homebrew, apt)
- No remote health checks beyond OpenAI/Ollama
- No config file reading in `braid-bootstrap` itself — that belongs to a future `braid-config` crate
