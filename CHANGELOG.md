# Changelog

All notable changes to braid are documented here. Follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- `braid-observe`: replay tests verifying `SessionWriter` round-trip via `ReplaySession`
  (`crates/braid-observe/src/replay.rs`)
- `xtask check-deps`: CI-enforced hexagonal crate-boundary check — detects illegal
  cross-crate imports at build time (`xtask/src/main.rs`)
- `AGENTS.md`: Codex/Oz operating guide for the braid workspace
- `scripts/preflight.nu`: local environment preflight checks for session start

### Changed
- Removed legacy `HANDOFF.md`, `HANDOFF.braid.workspace.yaml`, `scripts/tsi`, and
  `scripts/install-tsi.sh` (superseded by `hj` handoff journal)
- `openai.rs`: `validate_api_key` now takes `&str` (clippy `needless_pass_by_value`)

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
