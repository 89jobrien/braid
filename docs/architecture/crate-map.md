# Crate Map

## Workspace Members

| Crate | Kind | Role |
|---|---|---|
| `braid-model` | lib | Canonical domain types — single source of truth. Leaf crate. |
| `braid-core` | lib | Runtime engine, `Provider`/`Planner`/`ToolExecutor` traits, `SimpleLoopPlanner` |
| `braid-providers` | lib | Provider adapters: OpenAI, Ollama |
| `braid-redact` | lib | `RedactionPipeline` with ordered rule chain |
| `braid-hooks` | lib | `Hook` trait, `HookedExecutor`, destructive-command guard |
| `braid-mcp` | lib | MCP server over stdio (JSON-RPC 2.0). Only async crate (tokio). |
| `braid-observe` | lib | Session persistence, ingestion, replay, rendering |
| `braid-cli` | bin | Thin operator entrypoint — wires everything together |

## Dependency Graph

Arrows mean "depends on":

```
braid-cli
├── braid-observe   ──→ braid-model
├── braid-redact    ──→ braid-model
├── braid-hooks     ──→ braid-core  ──→ braid-model
├── braid-mcp       ──→ braid-model
└── braid-providers ──→ braid-core  ──→ braid-model
```

`braid-model` is the leaf. No crate except `braid-cli` depends on more than two others. `braid-core` knows nothing about providers, persistence, or redaction.

## Build Order (Phase Progression)

```
Phase 1 ──── braid-model ──── braid-core ──── braid-providers ──── braid-cli
                                                   (minimal runnable slice)

Phase 2 ────────────────── braid-redact ──── braid-hooks ──── braid-mcp
                                                   (safety + tool exposure)

Phase 3 ──────────────────────────────── braid-observe
                                                   (session observability)

Phase 4 ──── braid-context ──── braid-bootstrap ──── braid-tui   (planned)
```

## Crate Details

### `braid-model`

Pure data — no logic. All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`.

Key modules: `message`, `tool`, `provider`, `event`, `session`, `transcript`, `task`.

### `braid-core`

Three traits define the engine's extension surface:

- `Provider` — answers `complete(ProviderRequest) → ProviderResponse`
- `ToolExecutor` — answers `execute(ToolCall) → ToolResult`
- `Planner` — decides `next_action(SessionState) → Action`

`Engine<T: ToolExecutor, P: Provider>` is generic over both. The `SimpleLoopPlanner` implements the default tool-call loop.

### `braid-providers`

`OpenAiProvider` handles OpenAI wire format (and Ollama's compatible API). Synchronous HTTP via `reqwest::blocking`. No other crate does HTTP.

### `braid-redact`

Composable `RedactionRule` chain applied to `Message`s and `Event`s. Built-in rules: secret patterns, env vars, home paths. Wired into the engine via `Engine::with_redactor`.

### `braid-hooks`

`HookedExecutor<T>` wraps any `ToolExecutor` with pre/post hook gating. The `DestructiveCommandGuard` blocks dangerous shell patterns. Wired as the `T` in `Engine<T, P>`.

### `braid-mcp`

Standalone JSON-RPC 2.0 server over stdin/stdout. Exposes braid tools to MCP clients (Claude Code, etc.). `McpToolRegistry` is independent of `braid-core`'s `ToolRegistry`.

### `braid-observe`

Four sub-modules:
- `store` — `SessionStore` (batch r/w) + `SessionWriter` (streaming append)
- `ingest` — `Ingester` trait + `BraidIngester`, `ClaudeCodeIngester`, `DevloopIngester`
- `replay` — `ReplaySession` with 1-based indexed, payload-preserving event view
- `render` — `render_session()` → human-readable terminal output

### `braid-cli`

`clap`-based CLI. Subcommands: `run`, `doctor`, `mcp`, `sessions list/show/prune`. No business logic — delegates entirely to other crates.
