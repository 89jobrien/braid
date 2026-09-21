# braid-context

`braid-context` gathers current project and task information, filters stale data, compacts it to a
token budget, and exposes the result through Braid's `ContextProvider` and `ToolExecutor` ports.

## Workspace role and data flow

The crate bridges external context sources into `braid-model::ContextSnapshot` values:

```text
ContextSource(s) -> ContextAssembler -> ContextAssemblerProvider -> braid-engine
                                      -> RefreshContextTool
```

The engine can inject a rendered snapshot as a leading system message. Context acquisition failures
remain outside the domain model and are handled by the assembler or engine.

## Sources

| Source | Captured data | Staleness window |
| --- | --- | --- |
| `RepoSource` | `git diff --stat HEAD` and the last ten one-line commits | 30 minutes |
| `DoobSource` | `doob todo list --json`, optionally filtered by project | 1 hour |

Both commands have a five-second timeout and one-megabyte output cap. `DoobSource` returns no chunks
when Doob is absent, fails, or emits unusable JSON. `RepoSource` accepts an injected root path.

## Assembly and compaction

`ContextAssembler` defaults to a 2,000-token budget using the model's approximate byte-based token
estimate. It:

1. Fetches each source and ignores failed sources.
2. Removes chunks older than each source's staleness window.
3. Returns all chunks when the total is at most half the budget.
4. Above that threshold, asks an optional provider for a summary under 400 words.
5. If summarization is absent or fails, drops oldest chunks until the budget is met.

All sources failing or returning no fresh chunks is an error.

## Key APIs and usage

```rust
use braid_context::{ContextAssembler, DoobSource, RepoSource};

let assembler = ContextAssembler::new(vec![
    Box::new(DoobSource::with_project("braid")),
    Box::new(RepoSource::with_root(".")),
])
.with_budget(2_000);

let snapshot = assembler.assemble()?;
println!("{}", snapshot.render());
# Ok::<(), anyhow::Error>(())
```

- `ContextSource` is the extension trait for new collectors.
- `ContextAssemblerProvider` implements `braid_ports::ContextProvider` and retains the prior
  snapshot for rolling refresh.
- `RefreshContextTool` calls `refresh()` and returns rendered markdown. With no provider configured,
  it returns `No context provider configured.`

## Implementation status

Collection, staleness filtering, provider-backed summarization, fallback trimming, engine injection,
and explicit refresh are implemented. Context is currently limited to Git and Doob sources. Source
failures are deliberately best-effort, so missing optional tools may appear as an empty-source error
rather than a source-specific diagnostic.

## Features

This crate has no Cargo feature flags.

## Development and testing

Unit tests use stub sources and providers. Ignored integration tests shell out to real Git and Doob:

```console
cargo nextest run -p braid-context
cargo test -p braid-context --test integration -- --ignored
cargo clippy -p braid-context -- -D warnings
```
