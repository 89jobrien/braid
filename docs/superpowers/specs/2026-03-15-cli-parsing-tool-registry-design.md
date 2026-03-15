# Design: CLI Parsing with Clap + Tool Registry

**Date**: 2026-03-15
**Status**: Approved
**Scope**: Phase 1 — add clap-based CLI subcommands and a tool registry to braid-core

---

## 1. CLI Parsing (braid-cli)

### Subcommands

**`braid run [PROMPT]`** — run a session against a provider.

- `PROMPT`: optional positional argument. If omitted, read from stdin.
- `--provider <mock|openai>`: provider selection. Default: `openai` if `OPENAI_API_KEY` is set, else `mock`.
- `--model <model>`: model name. Default: `gpt-4o`.

**`braid doctor`** — check environment health.

No `tool` subcommand yet — deferred until real tools exist.

### Dependencies

`braid-cli` gains `clap` (with `derive` feature).

Add `clap = { version = "4", features = ["derive"] }` to `[workspace.dependencies]` and reference it in `braid-cli/Cargo.toml`.

### CLI Structure

```rust
#[derive(Parser)]
#[command(name = "braid")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a session against a provider
    Run {
        /// Prompt text (reads stdin if omitted)
        prompt: Option<String>,
        /// Provider to use
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long, default_value = "gpt-4o")]
        model: String,
    },
    /// Check environment health
    Doctor,
}
```

### Provider Resolution

For `braid run`:
1. If `--provider mock`, use `MockProvider`.
2. If `--provider openai`, use `OpenAiProvider` with the given `--model`. Fail if `OPENAI_API_KEY` not set.
3. If `--provider` not specified: use `openai` if `OPENAI_API_KEY` is set, else `mock`.

### Prompt Resolution

1. If positional `PROMPT` is given, use it.
2. Else, read all of stdin as the prompt. If stdin is a TTY and empty, print a hint and exit.

---

## 2. Tool Registry (braid-core)

### Purpose

A named registry of tools that dispatches `ToolCall` by name. Implements `ToolExecutor` so it slots into `Engine<T, P>` without changing the generic.

### Interface

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, name: impl Into<String>, tool: Box<dyn ToolExecutor>);
    pub fn list(&self) -> Vec<&str>;
    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor>;
}

impl ToolExecutor for ToolRegistry {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
    // Looks up call.name in the registry, delegates to the matching tool.
    // Returns error if tool not found.
}
```

### ToolExecutor Trait Changes

`ToolExecutor` must become object-safe to store in `Box<dyn ToolExecutor>`. The current trait is already object-safe (single method taking `&self`).

### Dependencies

`braid-core` gains `std::collections::HashMap` (stdlib, no new crate deps).

---

## 3. Doctor Command

### Checks

1. **Rust toolchain**: run `rustc --version`, parse version, verify >= 1.88. Report version found.
2. **OPENAI_API_KEY**: check env var presence. Report set/not set (don't print the key).
3. **OpenAI connectivity**: if key is set, send a minimal chat completions request (single-token max_tokens) and report success/failure with error detail.
4. **Workspace health**: run `cargo check --workspace` and report pass/fail.

### Output Format

```
rust toolchain ... ok (1.88.0)
OPENAI_API_KEY ... set
openai connectivity ... ok
workspace health ... ok
```

Or on failure:
```
rust toolchain ... FAIL (found 1.78.0, need >= 1.88)
OPENAI_API_KEY ... not set
openai connectivity ... skipped (no API key)
workspace health ... FAIL (cargo check failed)
```

### Implementation

Doctor logic lives in `braid-cli` (not in core) — it's operator tooling, not runtime. It shells out for `rustc --version` and `cargo check`, uses `OpenAiProvider` for the connectivity check.

---

## Build Order

1. Add `clap` workspace dependency and basic CLI skeleton with `run`/`doctor` subcommands
2. Add `ToolRegistry` to `braid-core`
3. Wire up `run` subcommand with provider/model selection and stdin support
4. Implement `doctor` subcommand
5. Update CLI snapshot tests (if any) or add basic tests
