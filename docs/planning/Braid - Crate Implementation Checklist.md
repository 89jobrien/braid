---
type: research
source_type: plan
topic: [braid, rust, workspace, implementation]
status: approved
tags: [braid, rust, workspace, implementation, checklist]
---

# Summary

Concrete implementation checklist for **Braid**. This note turns the extraction map into crate-by-crate build phases, required deliverables, and donor scope.

# Phase 0: Workspace Lock

## Whole Workspace

- [ ] Create the Rust workspace root and member crates.
- [ ] Freeze crate boundaries from [Braid - Rust Workspace Spec](./Braid%20-%20Rust%20Workspace%20Spec.md).
- [x] Write ADRs for event envelope, tool contract, and component format.
- [ ] Set testing conventions for unit, contract, and integration coverage.
- [ ] Define the rule for donor extraction: reimplement ideas, do not port code blindly.

# Phase 1: Minimal Vertical Slice

## `braid-model`

Donors:
- `looprs`
- `message-extractor`
- `inflection`
- `goder`

Extract:
- canonical event envelope
- session IDs and lifecycle types
- tool request/response model
- provider-neutral input/output model
- task/context payload shapes

Checklist:
- [ ] Define `Event`, `Session`, `ToolCall`, `ToolResult`, `ProviderRequest`, and `ProviderResponse`.
- [ ] Define normalized message/transcript structures.
- [ ] Define task/context/import model without repo-specific leakage.
- [ ] Add serde round-trip tests for all public model types.

Definition of done:
- `braid-model` is the only source of shared runtime types.

## `braid-core`

Donors:
- `looprs`
- `neocode`
- `joecc`
- `code-mode-python`

Extract:
- runtime loop
- session lifecycle/state machine
- planner/executor split
- deterministic patch/validate pressure
- execution substrate boundaries

Checklist:
- [ ] Implement a session engine that can execute one prompt -> tool -> response cycle.
- [ ] Separate planner decisions from executor actions.
- [ ] Define tool execution traits and runtime interfaces.
- [ ] Add retry/validation boundaries without hardcoding provider logic.
- [ ] Add deterministic tests for one minimal run.

Definition of done:
- `braid-core` can run one reproducible session against a mock provider and one mock tool.

## `braid-providers`

Donors:
- `looprs`
- `goder`
- `lisa`

Extract:
- provider adapter boundary
- routing policy separation
- model/provider normalization

Checklist:
- [ ] Define provider adapter traits over `braid-model`.
- [ ] Implement `mock` provider first.
- [ ] Add one real provider adapter only after the mock path is stable.
- [ ] Keep routing policy outside the runtime loop.
- [ ] Add contract tests shared across providers.

Definition of done:
- `braid-core` can run unchanged against mock and one real provider adapter.

## `braid-cli`

Donors:
- `pj`
- `dotfiles`
- `slash`

Extract:
- operator front door
- command ergonomics
- compact command grammar if useful

Checklist:
- [ ] Implement `run`, `doctor`, and `tool` command groups.
- [ ] Make `run` execute the minimal vertical slice.
- [ ] Keep CLI parsing thin and delegate behavior to other crates.
- [ ] Add snapshot tests for CLI help/output shape.

Definition of done:
- one CLI command starts and observes a minimal Braid session locally.

# Phase 2: Safety + Tool Surface

## `braid-hooks`

Donors:
- `hooks`
- `ctx`
- `threesheets`
- `maestro`

Extract:
- hook contract
- guard registry
- destructive-command protections
- freshness/staleness gates
- standalone guard execution

Checklist:
- [ ] Define pre/post execution hook contracts.
- [ ] Implement config-driven hook registration.
- [ ] Port destructive-command blocking as a Rust-native guard.
- [ ] Add a freshness/context guard for stale inputs.
- [ ] Make deny messages actionable, not generic.
- [ ] Add contract tests for fail-open vs fail-closed behavior.

Definition of done:
- risky tool invocations are blocked predictably with clear reasons and remediation hints.

## `braid-mcp`

Donors:
- `personal-mcp`
- `mcp-joecc`
- `code-mode-python`

Extract:
- MCP server boundary
- stable tool registration
- adapter/sync orchestration ideas
- separation between tool exposure and orchestration

Checklist:
- [ ] Define MCP-facing tool registry interfaces.
- [ ] Expose one built-in tool over MCP.
- [ ] Keep MCP request handling separate from runtime state management.
- [ ] Add schema/contract tests for exposed tools.

Definition of done:
- one Braid tool is callable through MCP without duplicating orchestration logic.

## `braid-redact`

Donors:
- `obfsck`

Extract:
- path/token/secret redaction
- redact-before-persist policy
- library-grade scrubber interfaces

Checklist:
- [ ] Define text and path redaction pipelines.
- [ ] Add composable redaction rules.
- [ ] Ensure event and transcript types can be scrubbed before storage.
- [ ] Add regression tests for representative sensitive inputs.

Definition of done:
- `braid-observe` and `braid-mcp` can depend on redaction as a library, not copy logic.

# Phase 3: Observability + Context

## `braid-observe`

Donors:
- `devloop`
- `message-extractor`
- `inflection`
- `gaw`
- `peeprs`
- `roxy`
- `tbh`

Extract:
- ingest vs normalize vs index separation
- trace/replay workflow
- local event search if justified
- operator-facing run inspection

Checklist:
- [ ] Build ingest pipeline from runtime events.
- [ ] Normalize transcripts/messages before indexing.
- [ ] Add replay support for one stored session.
- [ ] Add a minimal TUI or textual run inspector.
- [ ] Add search/index only if real event volume justifies it.

Definition of done:
- a completed run can be inspected and replayed from stored events.

## `braid-context`

Donors:
- `doob`
- `kan`
- `threesheets`
- `neocode`
- `lisa`

Extract:
- task import
- bounded context snapshots
- freshness windows
- compaction pressure
- syntax-aware extraction only where it materially helps

Checklist:
- [x] Define snapshot and compaction interfaces.
- [x] Add import path for one local task source.
- [x] Add staleness metadata to context inputs.
- [x] Add bounded compaction so context cannot grow without pressure.
- [x] Keep extraction selective; do not build a giant ingestion framework.

Definition of done:
- Braid can produce a bounded, timestamped context package from local work state.

# Phase 4: Operator Layer + Content Layer

## `braid-bootstrap`

Donors:
- `dotfiles`
- `pj`
- `joecc`
- `maestro`

Extract:
- machine doctor/install flows
- environment discovery
- secrets/tooling checks
- operator-controlled setup paths

Checklist:
- [ ] Implement `doctor` checks for required tools and env vars.
- [ ] Implement one install/setup flow for local development.
- [ ] Keep system interaction wrapped behind explicit commands.
- [ ] Avoid inheriting shell-sprawl as the default architecture.

Definition of done:
- a new machine can be checked and brought to a runnable Braid state with explicit commands.

## `braid-components`

Donors:
- `steve`
- `looprs`
- `slash`

Extract:
- manifest format
- prompt/template packaging
- loader/registry model
- optional workflow grammar ideas

Checklist:
- [ ] Define manifest schema for commands, skills, prompts, and templates.
- [ ] Implement loader and registry interfaces.
- [ ] Add one built-in component package.
- [ ] Refuse warehouse sprawl: only support components with a runtime consumer.

Definition of done:
- Braid can load one real command/skill bundle from a defined component manifest.

# Phase 5: Tightening Pass

## Cross-Crate Hardening

- [ ] Add contract tests across `braid-core`, `braid-hooks`, and `braid-mcp`.
- [ ] Add redaction coverage for all persisted event/transcript paths.
- [ ] Add replay tests for observed sessions.
- [ ] Add architecture checks to prevent crate-boundary drift.
- [ ] Remove any feature that exists only because a donor repo had it.

# Initial Execution Order

1. `braid-model`
2. `braid-core`
3. `braid-providers`
4. `braid-cli`
5. `braid-hooks`
6. `braid-mcp`
7. `braid-redact`
8. `braid-observe`
9. `braid-context`
10. `braid-bootstrap`
11. `braid-components`

# Criticisms / Improvement Areas

- This plan is only useful if it stays willing to cut features when the minimal vertical slice exposes bad assumptions.
- `braid-observe` and `braid-context` are the easiest places to overbuild because they can absorb endless “maybe useful later” ideas from donor repos.
- `maestro` and the work-only repos should remain constrained donors; if they start steering the architecture, the platform will drift away from its real use case.

# Links

- [Braid](./Braid.md)
- [Braid - Rust Workspace Blueprint](./Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Braid - Rust Workspace Spec](./Braid%20-%20Rust%20Workspace%20Spec.md)
