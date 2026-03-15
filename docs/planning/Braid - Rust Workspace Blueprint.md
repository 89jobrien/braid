---
type: research
source_type: spec
topic: [north-star, rust, workspace, architecture]
status: approved
tags: [rust, workspace, platform, consolidation, architecture]
---

# Summary

The implementation decision is now explicit:

**Build Braid from scratch in Rust.**

That means:
- no polyglot merger
- no gradual codebase fusion
- no pretending old repos are partial implementations of the final product

The old repos are **design donors**, not the foundation.

# Core Decision

The target is a **single Rust-first workspace** for **Braid** that absorbs the best ideas from the repo ecosystem while enforcing clearer boundaries than the original repo sprawl ever had.

# What This Means

## Do Not Merge Old Repos

- Do not try to glue `looprs`, `hooks`, `personal-mcp`, `devloop`, `message-extractor`, and other repos together directly.
- Do not port repo-by-repo mechanically.
- Do not preserve accidental boundaries that came from language choice, timing, or unfinished experiments.

## Do Extract Concepts

Extract and reimplement:
- runtime model
- provider abstraction
- hook contract
- MCP tool exposure
- observability/event pipeline
- redaction
- context/task surfaces
- bootstrap/operator control plane

# Proposed Workspace Shape

## Top-Level Crates

### `braid-cli`

- operator entrypoint
- command tree
- front door for running sessions, tools, diagnostics, and local workflows

### `braid-core`

- agent loop
- session lifecycle
- tool execution contract
- command/skill/agent/rule dispatch

### `braid-model`

- canonical domain types
- events
- session state
- task/context data contracts
- provider-neutral request/response models

### `braid-providers`

- model/provider adapters
- routing logic
- future `cod`/`goder` lessons captured here, not as sibling runtimes

### `braid-hooks`

- Rust-native pre/post tool policy layer
- absorbs the strongest ideas from `hooks`, `ctx`, and `threesheets`
- should be able to run as standalone guard binaries if needed

### `braid-mcp`

- MCP server and tool exposure
- absorbs `personal-mcp` and `mcp-joecc` ideas
- must remain a tool surface, not a second orchestrator

### `braid-observe`

- event ingestion
- runtime traces
- replay and operator views
- absorbs `devloop`, `message-extractor`, `inflection`, `peeprs`, `roxy`, `tbh` patterns

### `braid-redact`

- redaction and privacy boundary
- absorbs `obfsck` ideas
- should stay usable as a library by other crates

### `braid-context`

- context compaction
- bounded snapshotting
- import from local task/taskboard state
- absorbs `threesheets`, `kan`, `doob`, and `neocode` compaction ideas

### `braid-bootstrap`

- machine/operator/bootstrap layer
- captures the right long-term parts of `dotfiles` and `pj`
- should wrap shell/system tools deliberately instead of inheriting shell sprawl blindly

### `braid-components`

- component registry/loader for commands, skills, prompts, templates, and reusable assets
- absorbs `steve` lessons without becoming another warehouse of stale assets

# Donor Map

## Highest-Value Donors

- `looprs` -> extract the runtime loop, session lifecycle, command/skill/rule dispatch shape, and provider-neutral tool execution model.
- `hooks` -> extract the hook contract, guard registry shape, deny-with-guidance UX, and session lifecycle enforcement boundaries.
- `personal-mcp` -> extract MCP server/tool exposure boundaries, stable tool registration patterns, and transport/schema discipline.
- `obfsck` -> extract the redaction library boundary, secret/path scrubbing primitives, and redact-before-persist posture.
- `devloop` -> extract operator trace inspection, replay/debug workflows, and useful local observability UX.
- `message-extractor` -> extract transcript normalization, message segmentation, and raw-to-normalized event conversion.
- `inflection` -> extract indexing/audit event models, query-oriented storage ideas, and ingestion vs indexing separation.
- `doob` + `kan` -> extract task/state models, board/status projections, and agent-consumable context packaging.
- `dotfiles` + `pj` -> extract machine bootstrap, environment discovery, and operator front-door workflow patterns.

## Reference-Tier Donors

- `neocode` -> deterministic patch/validate loop, small-diff discipline, and context compaction pressure.
- `joecc` -> planner/executor split, sandbox posture, and operator-controlled execution escalation.
- `goder` -> provider routing policy and EventBus lessons, not another sibling runtime.
- `ctx` -> destructive-command guards, shell quality gates, and fail-closed command filtering.
- `threesheets` -> freshness windows and rotating context snapshot ideas.
- `gaw` -> FTS/search primitives only if Braid needs local event/context search.
- `lisa` -> tiny-classifier routing and syntax-aware extraction ideas.
- `code-mode-python` -> execution substrate abstractions and tool injection boundaries.
- `slash` -> command language/parser ideas if a compact workflow DSL proves useful.

## Constrained Work Donors

- `maestro` -> extract only generic runtime isolation, long-running session control, operator guardrails, and local orchestration patterns. Exclude all work-specific platform logic.
- `topdoc`, `ToptalVaultObsidian`, and `intvwr` -> use only the ideas already promoted into the inventory-only work extract notes; do not re-open them as canonical architecture drivers.

# Boundaries To Preserve In The New Workspace

## Hard Boundaries

- redaction stays separate from observability
- MCP stays separate from orchestration
- hooks stay separate from runtime internals
- bootstrap stays separate from task/context logic
- components/content stay separate from execution core

## Explicit Non-Goals

- Do not rebuild every old UI surface immediately
- Do not carry work-specific product logic from `maestro`
- Do not create three sibling runtimes again
- Do not let the bootstrap layer swallow the application layer

# Build Order

## Phase 0: Architecture Lock

1. Freeze the crate boundaries above.
2. Define the canonical event model in Rust.
3. Define the canonical tool contract in Rust.
4. Define the canonical component format in Rust.

## Phase 1: Minimal Vertical Slice

1. `braid-model`
2. `braid-core`
3. `braid-cli`
4. `braid-providers`

Goal:
- one runnable Rust agent loop
- one provider path
- one tool contract

## Phase 2: Safety + Tooling

5. `braid-hooks`
6. `braid-mcp`
7. `braid-redact`

Goal:
- guarded tool execution
- MCP tool surface
- redaction as a library boundary

## Phase 3: Observability + Context

8. `braid-observe`
9. `braid-context`

Goal:
- traceable runs
- replayable events
- bounded context and task/state import

## Phase 4: Bootstrap + Components

10. `braid-bootstrap`
11. `braid-components`

Goal:
- one operator front door
- reusable commands/skills/components without warehouse sprawl

# First Features Worth Rebuilding

## Rebuild First

- loop-driven agent runtime
- provider abstraction
- pre/post tool guards
- MCP tool exposure
- event pipeline
- redaction
- context compaction
- task/context import from local tools

## Rebuild Later

- advanced UI experiments
- broad component catalogs
- exotic model-routing paths
- heavyweight training/evaluation surfaces

# Criticisms / Improvement Areas

- “Everything from scratch in Rust” is the right simplification only if you stay disciplined about crate boundaries. Otherwise you will just rebuild the same sprawl inside one workspace.
- The biggest danger is cargo-cult porting: reimplementing every old subsystem because it existed, not because it deserves a place in the final program.
- The only defensible path is to rebuild the vertical slices that prove the platform shape, then pull in donor ideas selectively.

# Links

- [Braid](./Braid.md)
- [Braid - Rust Workspace Spec](./Braid%20-%20Rust%20Workspace%20Spec.md)
- [Braid - Crate Implementation Checklist](./Braid%20-%20Crate%20Implementation%20Checklist.md)
- `Consolidation Strategy`
- `Active Project Review Follow-up Tasks`
- `Reference Tier Review Follow-up Tasks`
