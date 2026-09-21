# braid-cli

`braid-cli` is Braid's operator-facing command-line application. It composes the engine with
providers, redaction, hooks, context sources, session storage, components, bootstrap checks, and an
MCP stdio server.

## Workspace role

This is the composition root for the command-line product. Business logic remains in library
crates; this binary selects adapters and translates command-line input into library calls. The
package is not published.

`Braid` is the project and platform name. The Cargo package and installed executable are both named
`braid-cli`, and the live help reports `Usage: braid-cli <COMMAND>`. Invoke an installed build as
`braid-cli`; this package does not provide a `braid` executable. The workspace examples below use
`cargo run -p braid-cli -- ...` to select the same binary explicitly.

## Commands

| Command | Purpose |
| --- | --- |
| `run [PROMPT]` | Run one provider session; read stdin when the prompt is omitted |
| `doctor` | Run environment, credentials, connectivity, and workspace checks |
| `setup` | Create `$HOME/.braid/config.toml` without overwriting an existing file |
| `mcp` | Run the Content-Length-framed MCP server over stdio |
| `sessions list` | List stored session IDs, newest first |
| `sessions show ID` | Render a stored session timeline |
| `sessions prune --keep N` | Delete older sessions, keeping the newest `N` |
| `components list [--dir DIR]` | List filesystem components |
| `agent` | Run the same engine pipeline as a warpx JSON-lines harness |
| `completions` | Generate Nushell completions through the pre-Clap command path |

## Usage

```console
cargo run -p braid-cli -- run "Summarize this repository" --provider ollama --model qwen2.5:3b
printf 'Say hello' | cargo run -p braid-cli -- run --provider openai --model gpt-4o
cargo run -p braid-cli -- doctor
cargo run -p braid-cli -- setup
cargo run -p braid-cli -- sessions list
cargo run -p braid-cli -- sessions show 1712345678
cargo run -p braid-cli -- sessions prune --keep 50
cargo run -p braid-cli -- components list --dir ./components
cargo run -p braid-cli -- completions > braid-completions.nu
```

`run` and `agent` auto-select OpenAI when `OPENAI_API_KEY` exists; otherwise they use Ollama. The
default model is `gpt-4o`, so pass an Ollama model explicitly when needed.

Harness mode accepts `--prompt`, `--provider`, `--model`, and `--max-turns`. It uses `OZ_RUN_ID` as
the session ID when present, emits stored `Event` values as JSON lines, then emits a final
`{"type":"response", ...}` object.

## Runtime composition

- Sessions are stored beneath `$HOME/.braid/sessions`.
- Context comes from Doob todos and the current Git repository.
- A summarization provider is added when OpenAI credentials are available.
- `SecretPatternRule`, `EnvVarRule`, and `HomePathRule` redact provider-bound messages.
- `HookRegistry::fail_closed()` and `DestructiveCommandGuard` gate tool execution.
- The CLI registers `refresh_context` as an engine tool.
- The MCP command currently registers only the `echo` tool.

## Implementation status

All commands listed above are implemented. The generated configuration is currently owned by
`braid-bootstrap`; the `run` path constructs defaults directly rather than loading that file. MCP is
a small stdio server rather than a network daemon. The completion command is recognized before Clap
parsing and therefore is not displayed in top-level generated help.

## Features

This package has no Cargo feature flags and is `publish = false`.

## Development and testing

Cross-crate integration tests verify hook gating, redaction ordering, MCP error propagation, and
structured tool-input redaction.

```console
cargo nextest run -p braid-cli
cargo clippy -p braid-cli -- -D warnings
cargo check -p braid-cli
cargo run -p braid-cli -- --help
```
