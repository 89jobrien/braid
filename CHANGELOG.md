# Changelog

All notable changes to braid are documented here. Follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

---

## [0.3.0] — 2026-07-13

### Added
- `braid-components` (Phase 4): new crate implementing the component system
  - `ComponentManifest`: loads `component.toml` from a directory, validates all
    referenced files exist at load time (startup error, not runtime panic)
  - `FileSystemRegistry`: scans a root directory, loads every subdirectory
    containing `component.toml`, implements `ComponentRegistry`
  - Built-in `hello-world` component with a `greet` command and `system` prompt
  - `braid components list [--dir <path>]` CLI subcommand
- `braid-ports`: `ComponentRegistry` trait and `ComponentInfo` struct
- `docs/adrs/`: three Architecture Decision Records
  - `ADR-001-event-envelope-format.md` — JSONL event log, forward-compat skip,
    atomic `meta.json` rename
  - `ADR-002-tool-contract.md` — `ToolExecutor` port, `HookedExecutor<T>` composition,
    binary `HookVerdict`, string output rationale
  - `ADR-003-component-format.md` — proposed component manifest TOML format,
    `ComponentRegistry` port trait sketch (status: Proposed)

### Changed
- `dead_code = "warn"` added to `[workspace.lints.rust]` — donor audit found zero
  items to remove; lint makes the policy self-enforcing going forward
- Branch protection enabled on `main` (required checks: compile-check, test, audit, lint)

### Fixed
- `deny.toml`: removed invalid `allow-workspace` field in `[bans]` section (caused
  `cargo deny` to fail to deserialize config)
- `rustls-webpki` bumped `0.103.10 → 0.103.13` via `cargo update`, resolving
  RUSTSEC-2026-0098, RUSTSEC-2026-0099, RUSTSEC-2026-0104 (TLS cert name
  constraint validation bugs)

### Infra
- `xtask/Cargo.toml`: added `license.workspace = true` (cargo-deny was flagging
  xtask as unlicensed)
- `braid-providers/Cargo.toml`: `[package.metadata.cargo-machete] ignored = ["openssl"]`
  (vendored for musl cross-compilation, not a direct import)
- `deny.toml`: added `RUSTSEC-2026-0190` to ignore list (`Error::downcast_mut`
  unsoundness; no patched version available)

---

## [0.2.0] — 2026-07-12

### Added
- `braid-observe`: replay tests verifying `SessionWriter` round-trip via `ReplaySession`
  (`crates/braid-observe/src/replay.rs`)
- `xtask check-deps`: CI-enforced hexagonal crate-boundary check — detects illegal
  cross-crate imports at build time (`xtask/src/main.rs`)
- `AGENTS.md`: Codex/Oz operating guide for the braid workspace
- `scripts/preflight.nu`: local environment preflight checks for session start
- `docs/cli-contract.md`: operator-facing CLI contract — all subcommands, flags,
  env vars, exit codes, session model (JSONL), provider configuration, MCP mode
- `braid-tui`: state-machine ANSI escape stripper (`strip_ansi`) replacing the
  char-code approach that left escape sequence tails visible; applied in
  `render_timeline` and `render_detail`

### Changed
- Removed legacy `HANDOFF.md`, `HANDOFF.braid.workspace.yaml`, `scripts/tsi`, and
  `scripts/install-tsi.sh` (superseded by `hj` handoff journal)
- `openai.rs`: `validate_api_key` now takes `&str` (clippy `needless_pass_by_value`)

### Fixed
- `ci.yml`: integration-agent job called `./target/release/braid` — binary is
  `braid-cli`; corrected

### Infra
- `ci.yml`: added `cargo xtask check-deps` step to compile-check job

---

## [0.1.1] — 2026-04-07

### Added
- `braid-bootstrap`: full `doctor` and `setup` subsystem — `Check` trait, tool/key/config/
  connectivity checks, TOML config, ANSI terminal renderer
- `braid-cli`: `doctor` and `setup` subcommands delegated to `braid-bootstrap`
- Provider contract CI job (`provider-contract`) gated on `OPENAI_API_KEY` secret
- Nightly `cargo-geiger` unsafe audit with artifact upload and step summary
- `.githooks/` directory wired via `core.hooksPath`; release-build gate in pre-commit

### Changed
- Workspace clippy lints tightened to `-D warnings`; all warnings resolved
- `unsafe_code = "deny"` enforced workspace-wide; test modules that call `set_var`/
  `remove_var` carry `#[allow(unsafe_code)]` on the `mod tests` block

### Fixed
- OpenSSL vendored for musl cross-compilation (`x86_64-unknown-linux-musl` targets)

---

## [0.1.0] — 2026-03-28

Initial workspace release. Four-crate Phase 1 vertical slice:

- `braid-model`: canonical domain types (`Message`, `ToolCall`, `Event`, `Session`, …)
- `braid-core`: `Engine<P,T,S,R,C>`, `SimpleLoopPlanner`, `ToolRegistry`
- `braid-providers`: `OpenAiProvider` (OpenAI + Ollama-compatible), `MockProvider`
- `braid-cli`: thin CLI with `run` and `mcp` subcommands

Phase 2 crates:

- `braid-redact`: `RedactionPipeline` with `SecretPatternRule`, `EnvVarRule`, `HomePathRule`
- `braid-hooks`: `Hook` trait, `HookedExecutor<T>`, `DestructiveCommandGuard`
- `braid-mcp`: JSON-RPC 2.0 stdio MCP server, `McpToolRegistry`, echo tool
- `braid-observe`: `SessionStore`, `SessionWriter`, `Ingester`, `render_session()`

Phase 3 crates:

- `braid-context`: `ContextAssembler`, `DoobSource`, `RepoSource`, two-stage compaction
- `braid-bootstrap`: scaffolded (full impl in v0.1.1)

Additional:

- `braid-tui`: Ratatui session inspector with timeline + detail panes
- `braid-ports`: port traits (`Provider`, `ToolExecutor`, `Redactor`, `EventSink`, …)

[Unreleased]: https://github.com/89jobrien/braid/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/89jobrien/braid/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/89jobrien/braid/releases/tag/v0.1.0
