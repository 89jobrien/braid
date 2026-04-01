---
type: design
topic: [braid, context, phase3]
status: approved
tags: [braid, context, compaction, summarization, hexagonal]
---

# braid-context Design

**Date:** 2026-04-01
**Phase:** 3 — Observability + Context
**Status:** Approved

## Summary

`braid-context` is a context assembly crate that collects bounded, timestamped snapshots of local work state (tasks, repo diff) and injects them into provider requests. It sits between `braid-ports` and `braid-core`, providing a `ContextProvider` port that the engine calls at session start and on-demand via a `refresh_context` tool.

## Architecture

### Crate boundary

`braid-context` defines:

- `ContextSource` trait — produces `Vec<ContextChunk>` from an external source
- `ContextAssembler` — collects sources, filters, compacts, and returns `ContextSnapshot`
- `ContextSnapshot` — the output type injected into `ProviderRequest` as a system message prefix
- `ContextSummary` — LLM-generated rolling summary replacing raw chunks in long sessions
- Two built-in sources: `DoobSource` and `RepoSource`
- A new `ContextProvider` port trait added to `braid-ports`

### Dependency graph addition

```
braid-core → braid-ports (ContextProvider)
braid-context → braid-ports (Provider, EventSink)
braid-context → braid-model
```

`braid-context` does not depend on `braid-core`. The engine depends on the `ContextProvider` port; `braid-context` implements it.

## Core Types

```rust
pub struct ContextChunk {
    pub source: &'static str,       // e.g. "doob", "repo"
    pub label: String,               // human-readable label
    pub content: String,
    pub captured_at: DateTime<Utc>,
    pub token_estimate: usize,       // character_count / 4
}

pub struct ContextSummary {
    pub content: String,
    pub summarized_at: DateTime<Utc>,
    pub source_chunk_count: usize,
    pub token_estimate: usize,
}

pub struct ContextSnapshot {
    pub chunks: Vec<ContextChunk>,
    pub summary: Option<ContextSummary>,
    pub assembled_at: DateTime<Utc>,
    pub token_estimate: usize,       // sum of live chunks + summary
    pub dropped_chunks: usize,       // trimmed by budget — surfaced to operator
}

pub trait ContextSource {
    fn name(&self) -> &'static str;
    fn staleness_window(&self) -> Duration;
    fn fetch(&self) -> Result<Vec<ContextChunk>>;
}
```

Token estimation: `content.len() / 4`. No external tokenizer dependency.

## Built-in Sources

### `DoobSource`

- Runs `doob todo list --format json` as a subprocess
- Filters to todos matching the current project path
- `staleness_window`: 1 hour
- Failure mode: non-fatal — logs warning, returns empty vec

### `RepoSource`

- Runs `git diff --stat HEAD` + `git log --oneline -10`
- Captures current working directory as repo root
- `staleness_window`: 30 minutes
- Failure mode: non-fatal

## Data Flow

```
Session start OR refresh_context tool call
    │
    ▼
ContextAssembler::assemble(budget: TokenBudget)
    ├─ DoobSource::fetch()   → Vec<ContextChunk>
    └─ RepoSource::fetch()   → Vec<ContextChunk>
    │
    ▼
Staleness filter: drop chunks older than source.staleness_window()
    │
    ▼
Compaction (two modes — see below)
    │
    ▼
ContextSnapshot { chunks, summary, assembled_at, token_estimate, dropped_chunks }
    │
    ▼
Rendered as system message prefix → injected into ProviderRequest
```

## Compaction

Two-stage, applied in order:

### Stage 1 — Staleness filter
Each source declares `staleness_window`. Chunks older than their source's window are dropped before token counting.

### Stage 2 — Token budget (two modes)

**Short sessions** (total token estimate ≤ 50% of budget):
Staleness filter only. No further action.

**Long sessions** (total token estimate > 50% of budget):
The assembler calls the `Provider` to summarize the current snapshot into a `ContextSummary`. The summary replaces raw chunks in the snapshot. On each subsequent `refresh_context`, new chunks are appended to the summary content and a new summarization pass runs — a rolling handoff.

The assembler accepts `Option<Arc<dyn Provider>>` at construction:
- `Some(provider)` — uses LLM summarization for long sessions
- `None` — falls back to oldest-first drop (graceful degradation)

`ContextSnapshot.dropped_chunks` is always populated so the engine can log "N context items were trimmed."

**Default budget:** 2000 tokens (leaves headroom for the conversation itself).

## `ContextProvider` Port

Added to `braid-ports`:

```rust
pub trait ContextProvider {
    fn assemble(&self) -> Result<ContextSnapshot>;
    fn refresh(&self) -> Result<ContextSnapshot>;
}
```

The engine calls `assemble()` at session start and `refresh()` when the `refresh_context` tool is invoked.

## Error Handling

- `ContextSource::fetch()` → `Result<Vec<ContextChunk>>`: source failures are **non-fatal**. The assembler logs a warning via `EventSink` and continues with remaining sources.
- `ContextAssembler::assemble()` → `Result<ContextSnapshot>`: errors only if **all** sources fail and the snapshot is empty.
- Summarization failure: falls back to staleness+drop for that cycle; failure logged to the session event stream.

## Testing Strategy

### Unit tests (`braid-context`)
- `ContextAssembler` with in-memory `ContextSource` stubs
- Staleness filter: verify chunks beyond window are dropped
- Token budget trim: verify oldest-first drop when over budget without provider
- Summarization trigger: mock `Provider` returns fixed summary string; verify `ContextSummary` populated and replaces raw chunks
- Source failure: one stub fails; assembler succeeds with remaining source

### Contract tests
- Any `ContextSource` impl must return chunks with valid timestamps and non-zero token estimates
- `ContextAssembler` must always populate `dropped_chunks` (0 if nothing dropped)

### Integration test (`#[ignore]`)
- Wires real `DoobSource` + `RepoSource` against the braid repo
- Requires: doob installed, git repo present
- Verifies snapshot is non-empty and token estimate is reasonable

No snapshot tests for context content — too brittle against live system state.

## Checklist (from implementation plan)

- [ ] Define `ContextChunk`, `ContextSnapshot`, `ContextSummary`, `ContextSource` trait
- [ ] Add `ContextProvider` trait to `braid-ports`
- [ ] Implement `ContextAssembler` with staleness filter + token budget
- [ ] Implement rolling LLM summarization path
- [ ] Implement `DoobSource`
- [ ] Implement `RepoSource`
- [ ] Wire `ContextProvider` into engine session start
- [ ] Add `refresh_context` tool
- [ ] Unit tests (assembler stubs, compaction modes, summarization mock)
- [ ] Contract tests for `ContextSource`
- [ ] Integration test (`#[ignore]`)

## Non-Goals

- No persistence of snapshots between sessions (ephemeral assembly only)
- No syntax-aware extraction (keep it selective — no giant ingestion framework)
- No search/index over context history
- No support for remote task sources in this phase
