# Braid Architecture

This folder documents the braid agent platform — its crate structure, data models, and runtime data flows.

## Documents

| File | Description |
|---|---|
| [crate-map.md](crate-map.md) | All crates, their roles, and the dependency graph |
| [data-model.md](data-model.md) | Core domain types from `braid-model` |
| [engine-loop.md](engine-loop.md) | How `Engine::run()` works and event emission |
| [cmd-run-flow.md](cmd-run-flow.md) | End-to-end data flow through `braid run` |
| [session-persistence.md](session-persistence.md) | How sessions are written, stored, and replayed |
| [extension-points.md](extension-points.md) | Traits, ports, and how to extend braid |

## Quick Summary

Braid is a **personal agent platform** built as a Rust workspace. The architecture follows hexagonal design: a pure domain core (`braid-model`, `braid-core`) surrounded by adapters (providers, tools, hooks, redaction, persistence) wired together by a thin CLI.

```
braid-cli  ←→  braid-core  ←→  braid-model
               ↕    ↕    ↕
          providers hooks observe
```
