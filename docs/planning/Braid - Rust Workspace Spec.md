---
type: research
source_type: spec
topic: [rust, workspace, spec, architecture]
status: approved
tags: [rust, workspace, spec, platform, architecture]
---

# Summary

Concrete spec for the new Rust workspace that will implement **Braid** from scratch. This note defines the initial file tree, crate boundaries, and the role of each crate.

# Goals

- One Rust-first workspace
- Hard architectural boundaries between runtime, hooks, MCP, observability, redaction, context, and bootstrap
- Minimal vertical slice first, then staged expansion
- Old repos treated as idea donors only

# Proposed File Tree

```text
braid/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── justfile
├── .gitignore
├── docs/
│   ├── architecture/
│   │   ├── workspace-overview.md
│   │   ├── event-model.md
│   │   ├── tool-contract.md
│   │   ├── component-format.md
│   │   └── provider-contract.md
│   ├── decisions/
│   │   ├── 0001-crate-boundaries.md
│   │   ├── 0002-event-envelope.md
│   │   ├── 0003-tool-execution-contract.md
│   │   └── 0004-mcp-boundary.md
│   └── donors/
│       ├── looprs-extract.md
│       ├── hooks-extract.md
│       ├── personal-mcp-extract.md
│       ├── observe-pipeline-extract.md
│       ├── context-extract.md
│       └── reference-donors-extract.md
├── crates/
│   ├── braid-cli/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── braid-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine/
│   │       ├── session/
│   │       ├── tools/
│   │       ├── commands/
│   │       ├── skills/
│   │       ├── agents/
│   │       └── rules/
│   ├── braid-model/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── event.rs
│   │       ├── session.rs
│   │       ├── tool.rs
│   │       ├── provider.rs
│   │       ├── context.rs
│   │       └── task.rs
│   ├── braid-providers/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs
│   │       ├── anthropic.rs
│   │       ├── openai.rs
│   │       ├── ollama.rs
│   │       └── mock.rs
│   ├── braid-hooks/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── contract.rs
│   │       ├── registry.rs
│   │       ├── guards/
│   │       └── lifecycle/
│   ├── braid-mcp/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs
│   │       ├── tools/
│   │       └── adapters/
│   ├── braid-observe/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ingest/
│   │       ├── normalize/
│   │       ├── replay/
│   │       ├── index/
│   │       └── tui/
│   ├── braid-redact/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── patterns/
│   │       ├── path.rs
│   │       └── text.rs
│   ├── braid-context/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compact.rs
│   │       ├── snapshot.rs
│   │       ├── import/
│   │       └── tasks/
│   ├── braid-bootstrap/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── doctor.rs
│   │       ├── install.rs
│   │       ├── secrets.rs
│   │       ├── tools.rs
│   │       └── machine.rs
│   └── braid-components/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── loader.rs
│           ├── manifest.rs
│           ├── markdown.rs
│           └── registry.rs
├── examples/
│   ├── minimal-session/
│   ├── tool-call-flow/
│   └── mcp-server/
├── fixtures/
│   ├── events/
│   ├── components/
│   ├── prompts/
│   └── transcripts/
└── tests/
    ├── integration/
    ├── contracts/
    └── fixtures/
```

# Project Extraction Map

## Primary Donors

- `looprs` -> `braid-core`, `braid-model`, `braid-components`
  Extract: loop/session state machine, command-skill-rule wiring, provider-neutral tool execution, streaming/event boundaries, and the useful parts of the component loading model.
- `hooks` -> `braid-hooks`
  Extract: pre/post hook contract, config-driven hook registry, standalone guard executables, actionable deny messages, and session lifecycle enforcement patterns.
- `personal-mcp` -> `braid-mcp`
  Extract: MCP server boundary, tool registration discipline, transport/tool-schema separation, and the habit of keeping tool exposure separate from orchestration.
- `devloop` -> `braid-observe`
  Extract: operator-facing trace views, run inspection workflow, replay-oriented diagnostics, and live/debug UX for local runs.
- `message-extractor` -> `braid-observe`, `braid-model`
  Extract: transcript normalization, message segmentation, role/content cleanup, and a clean boundary between raw logs and normalized conversation events.
- `obfsck` -> `braid-redact`
  Extract: reusable secret/path/token redaction passes, redact-before-persist posture, and library-grade privacy boundaries instead of app-local scrubbers.
- `inflection` -> `braid-observe`, `braid-model`
  Extract: audit/index event model, query-oriented storage shape, and the distinction between ingestion, indexing, and operator-facing views.
- `doob` and `kan` -> `braid-context`
  Extract: local task model, status/board projections, task import surfaces, and the idea that agent context should be fed from live local work state instead of ad hoc notes.
- `dotfiles` and `pj` -> `braid-bootstrap`, `braid-cli`
  Extract: machine doctor/install flows, environment discovery, operator command ergonomics, and the front-door workflow for local development/runtime control.

## Secondary Donors

- `neocode` -> `braid-core`, `braid-context`
  Extract: deterministic patch/validate loop, small-diff discipline, compaction pressure, and skepticism about uncontrolled agent output.
- `joecc` -> `braid-core`, `braid-context`, `braid-bootstrap`
  Extract: planner/executor split, sandbox posture, repo-aware execution choices, and operator-controlled escalation points.
- `goder` -> `braid-providers`, `braid-model`
  Extract: routing policy separation, EventBus-style event propagation lessons, and provider orchestration that does not leak into the runtime loop.
- `ctx` -> `braid-hooks`
  Extract: destructive-command guards, shell quality checks, command classification heuristics, and fail-closed safety posture for risky tool calls.
- `threesheets` -> `braid-context`, `braid-hooks`
  Extract: freshness windows, rotating context snapshots, and the idea that stale context should be treated as a first-class execution risk.
- `gaw` -> `braid-observe`
  Extract: search/index primitives only if Braid needs local FTS over events, transcripts, or extracted context.
- `lisa` -> `braid-context`, `braid-providers`
  Extract: tiny-classifier routing ideas, structured extraction from code, and selective use of syntax-aware context building.
- `code-mode-python` -> `braid-core`, `braid-mcp`
  Extract: tool injection boundaries, sandbox/tool abstraction ideas, and the split between execution substrate and higher-level orchestration.
- `slash` -> `braid-cli`, `braid-components`
  Extract: command language/parser ideas if Braid needs a compact workflow or command DSL.
- `steve` -> `braid-components`
  Extract: manifests, prompt/template packaging, and component inventory structure without carrying over warehouse sprawl.

## Constrained Donors

- `maestro` -> `braid-core`, `braid-hooks`, `braid-bootstrap`
  Extract only generic ideas: long-running session control, runtime isolation patterns, operator guardrails, and local/dev environment orchestration. Do not import work-specific product logic, APIs, or platform assumptions.
- Inventory-only work repos like `topdoc`, `ToptalVaultObsidian`, and `intvwr`
  Extract only proven patterns already captured in the work-repo extract notes: service boundaries, sync/validation gates, critique-revision loops, and scoring/evaluation structure.

# Crate Descriptions

## `braid-cli`

Primary binary and operator front door.

Responsibilities:
- parse CLI commands
- start sessions
- invoke tools/workflows
- expose diagnostics and admin commands
- remain thin over `braid-core`

Should not own:
- provider logic
- hook logic
- event indexing

## `braid-core`

The runtime heart of the system.

Responsibilities:
- tool loop execution
- session lifecycle
- command/skill/agent/rule dispatch
- orchestration state transitions
- execution policies that are intrinsic to the runtime

Should absorb ideas from:
- `looprs`
- `cod`
- `goder`
- `neocode`
- `slash`

## `braid-model`

Canonical shared domain model.

Responsibilities:
- event envelope
- tool request/response shapes
- provider-neutral inference types
- context/task/session types
- shared IDs and metadata

Rule:
- all other crates should depend on this instead of inventing parallel domain types

## `braid-providers`

Provider abstraction and routing layer.

Responsibilities:
- provider adapters
- model selection/routing
- retries/backoff
- mock providers for tests

Should absorb ideas from:
- `goder`
- `cod`
- `lisa`

## `braid-hooks`

Externalized policy and guard layer in Rust.

Responsibilities:
- pre/post tool hook contract
- guard registry
- standalone hook binaries if needed
- shell/tool/file safety policies

Should absorb ideas from:
- `hooks`
- `ctx`
- `threesheets`

Rule:
- hooks enforce and observe; they do not become the main runtime

## `braid-mcp`

Stable MCP-facing tool layer.

Responsibilities:
- MCP server
- tool registration
- adapter boundaries for external/local systems
- typed tool results

Should absorb ideas from:
- `personal-mcp`
- `mcp-joecc`

Rule:
- tool exposure only, not orchestration

## `braid-observe`

Observability and replay system.

Responsibilities:
- ingest events and transcripts
- normalize multiple event shapes
- replay/rich inspection
- optional TUI/operator views
- indexing and audit support

Should absorb ideas from:
- `devloop`
- `message-extractor`
- `inflection`
- `peeprs`
- `roxy`
- `tbh`

Rule:
- normalization, replay, and indexing should stay internally separated even if they live in one crate first

## `braid-redact`

Privacy boundary and reusable sanitization library.

Responsibilities:
- PII/secret redaction
- path sanitization
- stable obfuscation helpers
- redaction profiles/levels

Should absorb ideas from:
- `obfsck`

Rule:
- keep this usable as a dependency by `braid-observe`, `braid-mcp`, and `braid-core`

## `braid-context`

Context shaping and local task import.

Responsibilities:
- context compaction
- bounded snapshots
- import state from local task/taskboard tools
- task/context transformation for runtime use

Should absorb ideas from:
- `doob`
- `kan`
- `threesheets`
- `neocode`
- `joecc`

## `braid-bootstrap`

Machine and operator bootstrap layer.

Responsibilities:
- doctor/install flows
- secrets/runtime/tool setup
- machine capability checks
- operator workflows previously spread across shell scripts

Should absorb ideas from:
- `dotfiles`
- `pj`

Rule:
- wrap external tooling deliberately; do not recreate unstructured shell sprawl in Rust

## `braid-components`

Reusable content/component system.

Responsibilities:
- load commands/skills/prompts/templates
- validate component manifests
- register and query components
- support external component libraries later

Should absorb ideas from:
- `steve`
- `slash`

Rule:
- this is a registry/loader, not an endless warehouse

# Initial Dependency Direction

```text
braid-model
├── braid-core
├── braid-providers
├── braid-hooks
├── braid-mcp
├── braid-observe
├── braid-redact
├── braid-context
├── braid-bootstrap
└── braid-components

braid-core
├── braid-cli
├── braid-observe
└── braid-context

braid-redact
├── braid-observe
└── braid-mcp
```

# First Implementation Slice

The first runnable slice should be:

1. `braid-model`
2. `braid-providers`
3. `braid-core`
4. `braid-cli`

Capabilities:
- one session type
- one provider
- one tool contract
- one built-in tool
- one event envelope

Only after that:
- `braid-hooks`
- `braid-mcp`
- `braid-redact`

# Non-Goals For V1

- rebuilding every old UI
- carrying over work-specific product logic
- broad component catalogs on day one
- full bootstrap parity with `dotfiles`
- full observability UI before the event model is stable

# Criticisms / Improvement Areas

- This spec is intentionally broad enough to cover the whole target program, which means it can still become a dumping ground unless the V1 slice is enforced hard.
- Some boundaries may later deserve separate workspaces rather than separate crates, but that should be driven by pressure from a working system, not by premature purity.
- The main risk is still overbuilding: if you start implementing all crates at once, you will reproduce the old ecosystem’s parallel-center-of-gravity problem inside one repo.

# Links

- [Braid - Rust Workspace Blueprint](./Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Braid - Crate Implementation Checklist](./Braid%20-%20Crate%20Implementation%20Checklist.md)
- [Braid](./Braid.md)
- `Consolidation Strategy`
