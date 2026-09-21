# braid-bootstrap

`braid-bootstrap` provides local environment setup, typed configuration, health checks, and terminal
rendering for Braid. It backs the CLI's `setup` and `doctor` commands.

## Workspace role

Bootstrap concerns stay outside the engine and domain crates. This crate may inspect the host,
invoke developer tools, probe provider endpoints, create Braid's configuration directory, and report
operator-facing diagnostics.

## Key APIs

| API | Purpose |
| --- | --- |
| `setup::run(path)` | Create the Braid directory and default `config.toml` idempotently |
| `doctor::run_checks()` | Execute the built-in health checks and return structured results |
| `Check`, `CheckResult`, `CheckStatus` | Extension contract and result model for checks |
| `TerminalRenderer` | Render results with `ok`, `warn`, and `FAIL` labels |
| `BraidConfig::load` | Deserialize configuration from TOML |
| `BraidConfig::default` | Produce the current default provider/session/context configuration |

The built-in doctor checks Rust 1.88+, Git, Doob, cargo-nextest, cargo-deny,
`OPENAI_API_KEY`, Ollama connectivity, OpenAI connectivity, `cargo check --workspace`, and the
existence of `$HOME/.braid`.

## Usage

```rust
use braid_bootstrap::{doctor, render::TerminalRenderer};

let results = doctor::run_checks();
TerminalRenderer::render(&results);
```

Create configuration in an injected directory:

```rust
use braid_bootstrap::{config::BraidConfig, setup};

let dir = tempfile::tempdir()?;
let braid_dir = dir.path().join(".braid");
setup::run(&braid_dir)?;
let config = BraidConfig::load(&braid_dir.join("config.toml"))?;
assert_eq!(config.provider.model, "gpt-4o");
# Ok::<(), anyhow::Error>(())
```

The generated defaults are OpenAI with `gpt-4o`, 20 maximum session turns, and a 2,000-token
context budget. Existing configuration files are never overwritten by `setup::run`.

## Implementation status

Setup, configuration loading, checks, and rendering are implemented. Several doctor checks execute
real commands or network requests and can be slow or environment-dependent. OpenAI connectivity
treats HTTP 429 as reachable; missing Ollama is a warning, while a missing OpenAI key is a failure
in the dedicated key check.

The CLI currently generates this configuration but does not use it to configure normal session
runs.

## Features

This crate has no Cargo feature flags.

## Development and testing

Unit tests use temporary directories for setup/configuration and cover result rendering. Some check
tests inspect or temporarily alter environment variables.

```console
cargo nextest run -p braid-bootstrap
cargo clippy -p braid-bootstrap -- -D warnings
cargo check -p braid-bootstrap
```
