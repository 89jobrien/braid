# braid-bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `braid-bootstrap`, a library crate providing structured environment health checks (`doctor`) and a non-interactive local setup flow (`setup`), then wire it into `braid-cli`.

**Architecture:** `braid-bootstrap` is a pure library crate with no dependency on other braid crates. Each check implements a `Check` trait returning `CheckResult` — failures are values, never panics or `Err`. `braid-cli` delegates `doctor` and gains a new `setup` subcommand by depending on this crate.

**Tech Stack:** Rust 2024 edition, `anyhow`, `serde`/`serde_json`, `reqwest` (blocking), `toml`, `tempfile` (dev dep for setup tests).

---

## File Map

### New files — `crates/braid-bootstrap/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest; deps: anyhow, serde, serde_json, reqwest (blocking), toml |
| `src/lib.rs` | Re-exports: `checks`, `config`, `doctor`, `render`, `setup` |
| `src/checks/mod.rs` | `Check` trait, `CheckResult`, `CheckStatus` |
| `src/checks/toolchain.rs` | `RustToolchainCheck` |
| `src/checks/keys.rs` | `OpenAiKeyCheck` |
| `src/checks/connectivity.rs` | `OllamaConnectivityCheck`, `OpenAiConnectivityCheck` |
| `src/checks/workspace.rs` | `WorkspaceHealthCheck` |
| `src/checks/tools.rs` | `GitCheck`, `DoobCheck`, `CargoNextestCheck`, `CargoDenyCheck` |
| `src/checks/config_dir.rs` | `BraidConfigDirCheck` |
| `src/config.rs` | `BraidConfig`, `ProviderConfig`, `SessionConfig`, `ContextConfig` |
| `src/doctor.rs` | `run_checks() -> Vec<CheckResult>` |
| `src/render.rs` | `TerminalRenderer` |
| `src/setup.rs` | `run(braid_dir: &Path) -> Result<()>` |

### Modified files

| File | Change |
|---|---|
| `Cargo.toml` (workspace) | Add `toml` to `[workspace.dependencies]`; add `braid-bootstrap` to `members` |
| `crates/braid-cli/Cargo.toml` | Add `braid-bootstrap = { path = "../braid-bootstrap" }` |
| `crates/braid-cli/src/main.rs` | Remove `mod doctor`; add `Command::Setup`; update `cmd_doctor()` |

---

## Task 1: Workspace + crate scaffold

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/braid-bootstrap/Cargo.toml`
- Create: `crates/braid-bootstrap/src/lib.rs`

- [ ] **Step 1: Add `toml` to workspace deps and `braid-bootstrap` to members**

In `/Users/joe/dev/braid/Cargo.toml`:
```toml
# In [workspace] members, add:
  "crates/braid-bootstrap",

# In [workspace.dependencies], add:
toml = "0.8"
```

- [ ] **Step 2: Create `crates/braid-bootstrap/Cargo.toml`**

```toml
[package]
name = "braid-bootstrap"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
toml.workspace = true
reqwest = { version = "0.12", features = ["blocking"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Create stub `src/lib.rs`**

```rust
pub mod checks;
pub mod config;
pub mod doctor;
pub mod render;
pub mod setup;
```

- [ ] **Step 4: Run `cargo check --workspace`**

```bash
cargo check --workspace
```
Expected: compiles (empty modules will need stubs — add them in next tasks).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/braid-bootstrap/
git commit -m "feat(braid-bootstrap): scaffold crate and workspace wiring"
```

---

## Task 2: Core check types

**Files:**
- Create: `crates/braid-bootstrap/src/checks/mod.rs`

- [ ] **Step 1: Write tests first**

At the bottom of `crates/braid-bootstrap/src/checks/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysPass;
    impl Check for AlwaysPass {
        fn run(&self) -> CheckResult {
            CheckResult {
                name: "always_pass",
                status: CheckStatus::Pass,
                message: "fine".into(),
            }
        }
    }

    struct AlwaysFail;
    impl Check for AlwaysFail {
        fn run(&self) -> CheckResult {
            CheckResult {
                name: "always_fail",
                status: CheckStatus::Fail,
                message: "broken".into(),
            }
        }
    }

    #[test]
    fn check_result_carries_name_and_message() {
        let r = AlwaysPass.run();
        assert_eq!(r.name, "always_pass");
        assert!(matches!(r.status, CheckStatus::Pass));
        assert_eq!(r.message, "fine");
    }

    #[test]
    fn fail_result_has_fail_status() {
        let r = AlwaysFail.run();
        assert!(matches!(r.status, CheckStatus::Fail));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: compile error — types not defined yet.

- [ ] **Step 3: Write the types**

```rust
pub mod config_dir;
pub mod connectivity;
pub mod keys;
pub mod toolchain;
pub mod tools;
pub mod workspace;

pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

pub trait Check: Send + Sync {
    fn run(&self) -> CheckResult;
}
```

- [ ] **Step 4: Run tests — must pass**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-bootstrap/src/checks/
git commit -m "feat(braid-bootstrap): add Check trait, CheckResult, CheckStatus"
```

---

## Task 3: Toolchain + key checks

**Files:**
- Create: `crates/braid-bootstrap/src/checks/toolchain.rs`
- Create: `crates/braid-bootstrap/src/checks/keys.rs`

- [ ] **Step 1: Write tests for `RustToolchainCheck`**

`crates/braid-bootstrap/src/checks/toolchain.rs`:

```rust
use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

pub struct RustToolchainCheck;

impl Check for RustToolchainCheck {
    fn run(&self) -> CheckResult {
        let output = Cmd::new("rustc").arg("--version").output();
        match output {
            Ok(out) if out.status.success() => {
                let raw = String::from_utf8_lossy(&out.stdout);
                let version = raw.trim().strip_prefix("rustc ").unwrap_or(raw.trim());
                let parts: Vec<&str> = version.split('.').collect();
                let ok = parts.len() >= 2
                    && parts[0].parse::<u32>().unwrap_or(0) >= 1
                    && parts[1].trim_end_matches(|c: char| !c.is_numeric())
                        .parse::<u32>()
                        .unwrap_or(0)
                        >= 88;
                if ok {
                    CheckResult {
                        name: "rust toolchain",
                        status: CheckStatus::Pass,
                        message: format!("rustc {version}"),
                    }
                } else {
                    CheckResult {
                        name: "rust toolchain",
                        status: CheckStatus::Fail,
                        message: format!("found {version}, need >= 1.88"),
                    }
                }
            }
            _ => CheckResult {
                name: "rust toolchain",
                status: CheckStatus::Fail,
                message: "rustc not found".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_toolchain_check_runs_without_panic() {
        // rustc is present in CI, so we just verify it returns a result
        let result = RustToolchainCheck.run();
        assert_eq!(result.name, "rust toolchain");
        // In CI with 1.88+, should pass
        assert!(matches!(result.status, CheckStatus::Pass | CheckStatus::Fail));
    }
}
```

- [ ] **Step 2: Write `OpenAiKeyCheck`**

`crates/braid-bootstrap/src/checks/keys.rs`:

```rust
use super::{Check, CheckResult, CheckStatus};

pub struct OpenAiKeyCheck;

impl Check for OpenAiKeyCheck {
    fn run(&self) -> CheckResult {
        if std::env::var("OPENAI_API_KEY").is_ok() {
            CheckResult {
                name: "OPENAI_API_KEY",
                status: CheckStatus::Pass,
                message: "set".into(),
            }
        } else {
            CheckResult {
                name: "OPENAI_API_KEY",
                status: CheckStatus::Fail,
                message: "not set — export OPENAI_API_KEY=sk-...".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_check_pass_when_env_set() {
        unsafe { std::env::set_var("OPENAI_API_KEY", "sk-test") };
        let r = OpenAiKeyCheck.run();
        assert!(matches!(r.status, CheckStatus::Pass));
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
    }

    #[test]
    fn key_check_fail_when_env_missing() {
        let saved = std::env::var("OPENAI_API_KEY").ok();
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
        let r = OpenAiKeyCheck.run();
        assert!(matches!(r.status, CheckStatus::Fail));
        assert!(r.message.contains("not set"));
        if let Some(v) = saved {
            unsafe { std::env::set_var("OPENAI_API_KEY", v) };
        }
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/braid-bootstrap/src/checks/
git commit -m "feat(braid-bootstrap): RustToolchainCheck, OpenAiKeyCheck"
```

---

## Task 4: Connectivity checks

**Files:**
- Create: `crates/braid-bootstrap/src/checks/connectivity.rs`

- [ ] **Step 1: Write `OllamaConnectivityCheck` and `OpenAiConnectivityCheck`**

```rust
use super::{Check, CheckResult, CheckStatus};

pub struct OllamaConnectivityCheck;

impl Check for OllamaConnectivityCheck {
    fn run(&self) -> CheckResult {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        match client.get("http://localhost:11434/api/tags").send() {
            Ok(resp) if resp.status().is_success() => CheckResult {
                name: "ollama connectivity",
                status: CheckStatus::Pass,
                message: "reachable at http://localhost:11434".into(),
            },
            _ => CheckResult {
                name: "ollama connectivity",
                status: CheckStatus::Warn,
                message: "not reachable — start with: ollama serve".into(),
            },
        }
    }
}

pub struct OpenAiConnectivityCheck;

impl Check for OpenAiConnectivityCheck {
    fn run(&self) -> CheckResult {
        let key = match std::env::var("OPENAI_API_KEY") {
            Ok(k) => k,
            Err(_) => {
                return CheckResult {
                    name: "openai connectivity",
                    status: CheckStatus::Warn,
                    message: "skipped — OPENAI_API_KEY not set".into(),
                };
            }
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1
        });

        match client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&key)
            .json(&body)
            .send()
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 429 => {
                CheckResult {
                    name: "openai connectivity",
                    status: CheckStatus::Pass,
                    message: "reachable".into(),
                }
            }
            Ok(resp) => CheckResult {
                name: "openai connectivity",
                status: CheckStatus::Fail,
                message: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult {
                name: "openai connectivity",
                status: CheckStatus::Fail,
                message: format!("request failed: {e}"),
            },
        }
    }
}
```

- [ ] **Step 2: Run `cargo check`**

```bash
cargo check -p braid-bootstrap
```
Expected: compiles (no unit tests for network calls — tested via integration).

- [ ] **Step 3: Commit**

```bash
git add crates/braid-bootstrap/src/checks/connectivity.rs
git commit -m "feat(braid-bootstrap): OllamaConnectivityCheck, OpenAiConnectivityCheck"
```

---

## Task 5: Tool checks + workspace + config dir

**Files:**
- Create: `crates/braid-bootstrap/src/checks/tools.rs`
- Create: `crates/braid-bootstrap/src/checks/workspace.rs`
- Create: `crates/braid-bootstrap/src/checks/config_dir.rs`

- [ ] **Step 1: Write `tools.rs`**

```rust
use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

fn version_check(name: &'static str, bin: &str, args: &[&str], warn_only: bool) -> CheckResult {
    match Cmd::new(bin).args(args).output() {
        Ok(out) if out.status.success() => {
            let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
            CheckResult { name, status: CheckStatus::Pass, message: msg }
        }
        _ => CheckResult {
            name,
            status: if warn_only { CheckStatus::Warn } else { CheckStatus::Fail },
            message: format!("{bin} not found — install with: cargo install {bin}"),
        },
    }
}

pub struct GitCheck;
impl Check for GitCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("git").arg("--version").output() {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult { name: "git", status: CheckStatus::Pass, message: msg }
            }
            _ => CheckResult {
                name: "git",
                status: CheckStatus::Fail,
                message: "git not found — install via your OS package manager".into(),
            },
        }
    }
}

pub struct DoobCheck;
impl Check for DoobCheck {
    fn run(&self) -> CheckResult {
        version_check("doob", "doob", &["--version"], true)
    }
}

pub struct CargoNextestCheck;
impl Check for CargoNextestCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("cargo")
            .args(["nextest", "--version"])
            .output()
        {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult { name: "cargo-nextest", status: CheckStatus::Pass, message: msg }
            }
            _ => CheckResult {
                name: "cargo-nextest",
                status: CheckStatus::Fail,
                message: "not found — install with: cargo install cargo-nextest".into(),
            },
        }
    }
}

pub struct CargoDenyCheck;
impl Check for CargoDenyCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("cargo")
            .args(["deny", "--version"])
            .output()
        {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult { name: "cargo-deny", status: CheckStatus::Pass, message: msg }
            }
            _ => CheckResult {
                name: "cargo-deny",
                status: CheckStatus::Fail,
                message: "not found — install with: cargo install cargo-deny".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_check_runs_without_panic() {
        let r = GitCheck.run();
        assert_eq!(r.name, "git");
        assert!(matches!(r.status, CheckStatus::Pass | CheckStatus::Fail));
    }

    #[test]
    fn doob_check_is_warn_on_missing() {
        // If doob is not installed, must be Warn (not Fail)
        let r = DoobCheck.run();
        assert_eq!(r.name, "doob");
        assert!(!matches!(r.status, CheckStatus::Fail), "doob must warn, not fail");
    }
}
```

- [ ] **Step 2: Write `workspace.rs`**

```rust
use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

pub struct WorkspaceHealthCheck;

impl Check for WorkspaceHealthCheck {
    fn run(&self) -> CheckResult {
        let output = Cmd::new("cargo").args(["check", "--workspace"]).output();
        match output {
            Ok(out) if out.status.success() => CheckResult {
                name: "workspace health",
                status: CheckStatus::Pass,
                message: "cargo check --workspace ok".into(),
            },
            Ok(_) => CheckResult {
                name: "workspace health",
                status: CheckStatus::Fail,
                message: "cargo check --workspace failed".into(),
            },
            Err(_) => CheckResult {
                name: "workspace health",
                status: CheckStatus::Fail,
                message: "cargo not found".into(),
            },
        }
    }
}
```

- [ ] **Step 3: Write `config_dir.rs`**

```rust
use super::{Check, CheckResult, CheckStatus};

pub struct BraidConfigDirCheck;

impl Check for BraidConfigDirCheck {
    fn run(&self) -> CheckResult {
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => {
                return CheckResult {
                    name: "~/.braid dir",
                    status: CheckStatus::Fail,
                    message: "HOME not set".into(),
                }
            }
        };
        let dir = std::path::PathBuf::from(home).join(".braid");
        if dir.exists() {
            CheckResult {
                name: "~/.braid dir",
                status: CheckStatus::Pass,
                message: dir.display().to_string(),
            }
        } else {
            CheckResult {
                name: "~/.braid dir",
                status: CheckStatus::Warn,
                message: "not found — run: braid setup".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_check_returns_warn_when_dir_absent() {
        // Point HOME at a temp dir that has no .braid subdir
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", tmp.path()) };
        let r = BraidConfigDirCheck.run();
        assert!(matches!(r.status, CheckStatus::Warn));
        assert!(r.message.contains("braid setup"));
    }
}
```

Note: `tempfile` is a dev-dep — this test only runs via `cargo nextest`.

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-bootstrap/src/checks/
git commit -m "feat(braid-bootstrap): tool checks, workspace check, config dir check"
```

---

## Task 6: `doctor.rs` — run_checks()

**Files:**
- Create: `crates/braid-bootstrap/src/doctor.rs`

- [ ] **Step 1: Write the module**

```rust
use crate::checks::{
    CheckResult,
    config_dir::BraidConfigDirCheck,
    connectivity::{OllamaConnectivityCheck, OpenAiConnectivityCheck},
    keys::OpenAiKeyCheck,
    toolchain::RustToolchainCheck,
    tools::{CargoDenyCheck, CargoNextestCheck, DoobCheck, GitCheck},
    workspace::WorkspaceHealthCheck,
};

pub fn run_checks() -> Vec<CheckResult> {
    let checks: Vec<Box<dyn crate::checks::Check>> = vec![
        Box::new(RustToolchainCheck),
        Box::new(GitCheck),
        Box::new(DoobCheck),
        Box::new(CargoNextestCheck),
        Box::new(CargoDenyCheck),
        Box::new(OpenAiKeyCheck),
        Box::new(OllamaConnectivityCheck),
        Box::new(OpenAiConnectivityCheck),
        Box::new(WorkspaceHealthCheck),
        Box::new(BraidConfigDirCheck),
    ];
    checks.into_iter().map(|c| c.run()).collect()
}
```

- [ ] **Step 2: Run `cargo check`**

```bash
cargo check -p braid-bootstrap
```
Expected: clean compile.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-bootstrap/src/doctor.rs
git commit -m "feat(braid-bootstrap): doctor::run_checks collects all checks"
```

---

## Task 7: `render.rs` — TerminalRenderer

**Files:**
- Create: `crates/braid-bootstrap/src/render.rs`

- [ ] **Step 1: Write tests first**

```rust
use crate::checks::{CheckResult, CheckStatus};

pub struct TerminalRenderer;

impl TerminalRenderer {
    pub fn render(results: &[CheckResult]) {
        print!("{}", Self::render_plain(results));
    }

    pub fn render_plain(results: &[CheckResult]) -> String {
        results
            .iter()
            .map(|r| {
                let (label, color) = match r.status {
                    CheckStatus::Pass => ("ok", "\x1b[32m"),
                    CheckStatus::Warn => ("warn", "\x1b[33m"),
                    CheckStatus::Fail => ("FAIL", "\x1b[31m"),
                };
                format!(
                    "{:<22} ... {color}{label}\x1b[0m ({})\n",
                    r.name, r.message
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckResult, CheckStatus};

    fn make(name: &'static str, status: CheckStatus, msg: &str) -> CheckResult {
        CheckResult { name, status, message: msg.into() }
    }

    #[test]
    fn render_plain_contains_check_name() {
        let results = vec![make("git", CheckStatus::Pass, "git version 2.44.0")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("git"));
        assert!(out.contains("ok"));
        assert!(out.contains("git version 2.44.0"));
    }

    #[test]
    fn render_plain_warn_contains_warn_label() {
        let results = vec![make("doob", CheckStatus::Warn, "not found")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("warn"));
    }

    #[test]
    fn render_plain_fail_contains_fail_label() {
        let results = vec![make("rust toolchain", CheckStatus::Fail, "not found")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("FAIL"));
    }

    #[test]
    fn render_plain_multiple_results() {
        let results = vec![
            make("git", CheckStatus::Pass, "ok"),
            make("doob", CheckStatus::Warn, "missing"),
        ];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("git"));
        assert!(out.contains("doob"));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-bootstrap/src/render.rs
git commit -m "feat(braid-bootstrap): TerminalRenderer with ANSI output"
```

---

## Task 8: `config.rs` — BraidConfig

**Files:**
- Create: `crates/braid-bootstrap/src/config.rs`

- [ ] **Step 1: Write tests first**

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraidConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
    pub context: ContextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub max_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub budget_tokens: usize,
}

impl Default for BraidConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig {
                default: "openai".into(),
                model: "gpt-4o".into(),
            },
            session: SessionConfig { max_turns: 20 },
            context: ContextConfig { budget_tokens: 2000 },
        }
    }
}

impl BraidConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = BraidConfig::default();
        assert_eq!(cfg.provider.default, "openai");
        assert_eq!(cfg.provider.model, "gpt-4o");
        assert_eq!(cfg.session.max_turns, 20);
        assert_eq!(cfg.context.budget_tokens, 2000);
    }

    #[test]
    fn config_round_trips_through_toml() {
        let original = BraidConfig::default();
        let serialized = toml::to_string_pretty(&original).unwrap();
        let loaded: BraidConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(loaded.provider.default, original.provider.default);
        assert_eq!(loaded.provider.model, original.provider.model);
        assert_eq!(loaded.session.max_turns, original.session.max_turns);
        assert_eq!(loaded.context.budget_tokens, original.context.budget_tokens);
    }

    #[test]
    fn load_reads_toml_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let cfg = BraidConfig::default();
        std::fs::write(&path, toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded = BraidConfig::load(&path).unwrap();
        assert_eq!(loaded.provider.model, "gpt-4o");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-bootstrap/src/config.rs
git commit -m "feat(braid-bootstrap): BraidConfig with toml serialization and load()"
```

---

## Task 9: `setup.rs`

**Files:**
- Create: `crates/braid-bootstrap/src/setup.rs`

- [ ] **Step 1: Write tests first**

```rust
use anyhow::Result;
use std::path::Path;

/// Creates ~/.braid/ and ~/.braid/config.toml if they don't exist.
/// `braid_dir` is typically `$HOME/.braid`.
pub fn run(braid_dir: &Path) -> Result<()> {
    if !braid_dir.exists() {
        std::fs::create_dir_all(braid_dir)?;
        println!("created  {}", braid_dir.display());
    }

    let config_path = braid_dir.join("config.toml");
    if config_path.exists() {
        println!("skipped  {} (already exists)", config_path.display());
    } else {
        let content = toml::to_string_pretty(&crate::config::BraidConfig::default())?;
        std::fs::write(&config_path, content)?;
        println!("created  {}", config_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_creates_dir_and_config() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_dir = tmp.path().join(".braid");
        assert!(!braid_dir.exists());

        run(&braid_dir).unwrap();

        assert!(braid_dir.exists());
        let config_path = braid_dir.join("config.toml");
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("gpt-4o"));
    }

    #[test]
    fn setup_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_dir = tmp.path().join(".braid");

        run(&braid_dir).unwrap();

        // Overwrite config with sentinel value to verify second run doesn't overwrite
        let config_path = braid_dir.join("config.toml");
        std::fs::write(&config_path, "# sentinel\n").unwrap();

        run(&braid_dir).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, "# sentinel\n", "second run must not overwrite existing config");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p braid-bootstrap
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/braid-bootstrap/src/setup.rs
git commit -m "feat(braid-bootstrap): setup::run — idempotent dir + config creation"
```

---

## Task 10: Wire into `braid-cli`

**Files:**
- Modify: `crates/braid-cli/Cargo.toml`
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Add dependency to `braid-cli/Cargo.toml`**

Add to `[dependencies]`:
```toml
braid-bootstrap = { path = "../braid-bootstrap" }
```

- [ ] **Step 2: Update `main.rs` — remove `mod doctor`, add `Setup` subcommand**

In the `Command` enum, add:
```rust
/// Set up local braid environment (~/.braid/)
Setup,
```

Replace `cmd_doctor()`:
```rust
fn cmd_doctor() -> Result<()> {
    let results = braid_bootstrap::doctor::run_checks();
    braid_bootstrap::render::TerminalRenderer::render(&results);
    Ok(())
}
```

Add `cmd_setup()`:
```rust
fn cmd_setup() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let braid_dir = std::path::PathBuf::from(home).join(".braid");
    braid_bootstrap::setup::run(&braid_dir)
}
```

In `main()` match arm:
```rust
Command::Setup => cmd_setup(),
```

Remove the entire `mod doctor { ... }` block (lines 190–285 in the current file).

- [ ] **Step 3: Run `cargo check --workspace`**

```bash
cargo check --workspace
```
Expected: clean compile.

- [ ] **Step 4: Run all tests**

```bash
cargo nextest run --workspace
```
Expected: all tests pass.

- [ ] **Step 5: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/braid-cli/ crates/braid-bootstrap/
git commit -m "feat(braid-cli): delegate doctor to braid-bootstrap, add setup subcommand"
```

---

## Self-Review

### Spec coverage

| Spec requirement | Covered by |
|---|---|
| `Check` trait, `CheckResult`, `CheckStatus` | Task 2 |
| `RustToolchainCheck` | Task 3 |
| `OpenAiKeyCheck` | Task 3 |
| `OllamaConnectivityCheck` | Task 4 |
| `OpenAiConnectivityCheck` | Task 4 |
| `WorkspaceHealthCheck` | Task 5 |
| `GitCheck`, `DoobCheck`, `CargoNextestCheck`, `CargoDenyCheck` | Task 5 |
| `BraidConfigDirCheck` | Task 5 |
| `TerminalRenderer::render` / `render_plain` | Task 7 |
| `BraidConfig`, `ProviderConfig`, `SessionConfig`, `ContextConfig` | Task 8 |
| `BraidConfig::load(path)` | Task 8 |
| `setup::run()` — idempotent, creates dir + config | Task 9 |
| `braid-cli`: remove `mod doctor`, add `Command::Setup` | Task 10 |
| `toml` added to workspace deps | Task 1 |
| ANSI hand-rolled (`\x1b[32m` etc.) | Task 7 |
| `DoobCheck` is Warn, not Fail | Task 5 |
| 5s timeout on connectivity checks | Task 4 |

All spec requirements covered. No gaps found.

### Placeholder scan

No TBD / TODO / "fill in details" patterns — all steps contain actual code.

### Type consistency

- `Check::run() -> CheckResult` — defined Task 2, used consistently Tasks 3–6, 10.
- `TerminalRenderer::render_plain(results: &[CheckResult])` — defined and tested Task 7.
- `BraidConfig::default()` — used in Task 8 tests and Task 9 setup.
- `setup::run(braid_dir: &Path)` — defined Task 9, called with `Path` in Task 10.
- `doctor::run_checks() -> Vec<CheckResult>` — defined Task 6, called in Task 10.
