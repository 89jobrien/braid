# CLI Parsing + Tool Registry Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add clap-based CLI with `run`/`doctor` subcommands and a tool registry to braid-core.

**Architecture:** Add clap derive-based CLI skeleton to braid-cli with `run` (provider/model selection, stdin support) and `doctor` (environment health checks). Add `ToolRegistry` to braid-core that dispatches by tool name and implements `ToolExecutor`. Wire CLI to use the registry.

**Tech Stack:** Rust 1.88, edition 2024, clap 4 (derive), std::process::Command for doctor

**Spec:** `docs/superpowers/specs/2026-03-15-cli-parsing-tool-registry-design.md`

---

## Chunk 1: Tool Registry + CLI Skeleton

### Task 1: Add ToolRegistry to braid-core

**Files:**
- Create: `crates/braid-core/src/registry.rs`
- Modify: `crates/braid-core/src/lib.rs`

- [ ] **Step 1: Write tests for ToolRegistry**

Create `crates/braid-core/src/registry.rs` with the struct and tests first:

```rust
use std::collections::HashMap;
use anyhow::{bail, Result};
use braid_model::{ToolCall, ToolResult};
use crate::tools::ToolExecutor;

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, tool: Box<dyn ToolExecutor>) {
        self.tools.insert(name.into(), tool);
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

impl ToolExecutor for ToolRegistry {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        let tool = self.tools.get(&call.name)
            .ok_or_else(|| anyhow::anyhow!("tool not found: {}", call.name))?;
        tool.execute(call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::StaticTool;

    #[test]
    fn register_and_list_tools() {
        let mut registry = ToolRegistry::new();
        registry.register("beta", Box::new(StaticTool::new("beta", "b output")));
        registry.register("alpha", Box::new(StaticTool::new("alpha", "a output")));
        assert_eq!(registry.list(), vec!["alpha", "beta"]);
    }

    #[test]
    fn execute_dispatches_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register("echo", Box::new(StaticTool::new("echo", "echoed")));
        let result = registry.execute(ToolCall {
            name: "echo".into(),
            input: "hello".into(),
        }).unwrap();
        assert_eq!(result.name, "echo");
        assert_eq!(result.output, "echoed");
    }

    #[test]
    fn execute_unknown_tool_errors() {
        let registry = ToolRegistry::new();
        let err = registry.execute(ToolCall {
            name: "missing".into(),
            input: "".into(),
        }).unwrap_err();
        assert!(err.to_string().contains("tool not found: missing"));
    }

    #[test]
    fn get_returns_tool_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register("echo", Box::new(StaticTool::new("echo", "out")));
        assert!(registry.get("echo").is_some());
        assert!(registry.get("missing").is_none());
    }
}
```

- [ ] **Step 2: Wire up lib.rs exports**

Add to `crates/braid-core/src/lib.rs`:

```rust
pub mod registry;

pub use registry::ToolRegistry;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p braid-core`
Expected: All tests pass (existing engine test + 4 new registry tests).

- [ ] **Step 4: Commit**

```bash
git add crates/braid-core/src/registry.rs crates/braid-core/src/lib.rs
git commit -m "feat: add ToolRegistry to braid-core"
```

### Task 2: Add clap dependency and CLI skeleton

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/braid-cli/Cargo.toml`
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Add clap workspace dependency**

In root `Cargo.toml`, add to `[workspace.dependencies]`:
```toml
clap = { version = "4", features = ["derive"] }
```

In `crates/braid-cli/Cargo.toml`, add to `[dependencies]`:
```toml
clap.workspace = true
```

- [ ] **Step 2: Replace main.rs with clap-based CLI**

Replace `crates/braid-cli/src/main.rs`:

```rust
use std::io::{self, IsTerminal, Read};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use braid_core::engine::Provider;
use braid_core::{Engine, RunInput, ToolRegistry};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::{MockProvider, OpenAiProvider};

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
        /// Provider to use (mock or openai; default: auto-detect)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long, default_value = "gpt-4o")]
        model: String,
    },
    /// Check environment health
    Doctor,
}

fn resolve_provider(flag: Option<&str>, model: &str) -> Result<Box<dyn Provider>> {
    let provider_name = match flag {
        Some(name) => name.to_string(),
        None => {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                "openai".into()
            } else {
                "mock".into()
            }
        }
    };

    match provider_name.as_str() {
        "mock" => Ok(Box::new(MockProvider)),
        "openai" => Ok(Box::new(OpenAiProvider::new(model)?)),
        other => bail!("unknown provider: {} (expected 'mock' or 'openai')", other),
    }
}

fn resolve_prompt(arg: Option<String>) -> Result<String> {
    if let Some(prompt) = arg {
        return Ok(prompt);
    }

    let stdin = io::stdin();
    if stdin.is_terminal() {
        bail!("no prompt provided. Usage: braid run \"your prompt\" or pipe via stdin");
    }

    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)
        .context("failed to read from stdin")?;

    if buf.trim().is_empty() {
        bail!("empty prompt from stdin");
    }

    Ok(buf)
}

fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let engine = Engine::new(ToolRegistry::new(), provider);
    let output = engine.run(RunInput {
        session_id: SessionId("session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: prompt }],
        }],
    })?;

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{}", response_text);
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    doctor::run_checks()
}

mod doctor {
    use anyhow::Result;
    use std::process::Command as ProcessCommand;

    pub fn run_checks() -> Result<()> {
        check_rust_toolchain();
        check_openai_key();
        check_openai_connectivity();
        check_workspace_health();
        Ok(())
    }

    fn check_rust_toolchain() {
        let output = ProcessCommand::new("rustc").arg("--version").output();
        match output {
            Ok(out) if out.status.success() => {
                let version_str = String::from_utf8_lossy(&out.stdout);
                let version = version_str.trim().strip_prefix("rustc ").unwrap_or(version_str.trim());
                // Parse major.minor
                let parts: Vec<&str> = version.split('.').collect();
                if parts.len() >= 2 {
                    let major: u32 = parts[0].parse().unwrap_or(0);
                    let minor: u32 = parts[1].parse().unwrap_or(0);
                    if major >= 1 && minor >= 88 {
                        println!("rust toolchain ... ok ({})", version);
                    } else {
                        println!("rust toolchain ... FAIL (found {}, need >= 1.88)", version);
                    }
                } else {
                    println!("rust toolchain ... FAIL (could not parse version: {})", version);
                }
            }
            _ => println!("rust toolchain ... FAIL (rustc not found)"),
        }
    }

    fn check_openai_key() {
        if std::env::var("OPENAI_API_KEY").is_ok() {
            println!("OPENAI_API_KEY ... set");
        } else {
            println!("OPENAI_API_KEY ... not set");
        }
    }

    fn check_openai_connectivity() {
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("openai connectivity ... skipped (no API key)");
            return;
        }

        use braid_core::engine::Provider;
        use braid_model::{ContentPart, Message, ProviderRequest, Role};
        use braid_providers::OpenAiProvider;

        match OpenAiProvider::new("gpt-4o") {
            Ok(provider) => {
                let request = ProviderRequest {
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "hi".into(),
                        }],
                    }],
                };
                match provider.complete(request) {
                    Ok(_) => println!("openai connectivity ... ok"),
                    Err(e) => println!("openai connectivity ... FAIL ({})", e),
                }
            }
            Err(e) => println!("openai connectivity ... FAIL ({})", e),
        }
    }

    fn check_workspace_health() {
        let output = ProcessCommand::new("cargo")
            .args(["check", "--workspace"])
            .output();
        match output {
            Ok(out) if out.status.success() => println!("workspace health ... ok"),
            Ok(_) => println!("workspace health ... FAIL (cargo check failed)"),
            Err(_) => println!("workspace health ... FAIL (cargo not found)"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { prompt, provider, model } => cmd_run(prompt, provider, model),
        Command::Doctor => cmd_doctor(),
    }
}
```

- [ ] **Step 3: Run cargo check and test**

Run: `cargo check --workspace && cargo test --workspace`
Expected: Compiles and all tests pass.

- [ ] **Step 4: Test CLI manually**

Run: `cargo run -p braid-cli -- run --provider mock "Hello world"`
Expected: Prints mock response.

Run: `echo "Hello from pipe" | cargo run -p braid-cli -- run --provider mock`
Expected: Prints mock response to piped prompt.

Run: `cargo run -p braid-cli -- doctor`
Expected: Prints health check output.

Run: `cargo run -p braid-cli -- --help`
Expected: Shows usage with `run` and `doctor` subcommands.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/braid-cli/Cargo.toml crates/braid-cli/src/main.rs
git commit -m "feat: add clap CLI with run and doctor subcommands"
```
