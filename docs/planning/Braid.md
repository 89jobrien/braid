---
type: research
source_type: spec
topic: [north-star, platform, consolidation, agents]
status: approved
tags: [platform, consolidation, agents, architecture, roadmap]
---

# Summary

**Braid** is the program you have been going for: a personal agent platform, not a loose collection of adjacent tools.

Implementation decision:
- build it from scratch in Rust
- treat old repos as idea donors, not merge targets

Its center of gravity is:
- `looprs` as the runtime/platform
- `hooks` as the external policy/guard layer
- `personal-mcp` as the tool-exposure layer
- `dotfiles` + `pj` as the machine/bootstrap control plane
- `devloop` + `message-extractor` + `obfsck` + `inflection` as the observability pipeline
- `doob` + `kan` as the task/context layer

Everything else should either feed Braid, remain a reference, or be archived.

# What The Program Is

This is not “another coding agent.”

It is a **full local agent operating environment** with:
- a Rust-native agent runtime
- hook-governed safety and workflow enforcement
- MCP-based tool exposure
- local-first context/task/state surfaces
- structured observability and replay
- reproducible machine bootstrap

# Canonical Shape

## 1. Runtime Core

- **Canonical repo**: `looprs`
- **Owns**: agent loop, providers, tools, commands, skills, agents, rules
- **Must absorb**:
  - `cod` minimalism and planning/context lessons
  - `goder` model routing and EventBus ideas
  - `neocode` deterministic loop, RL-pipeline, compaction, small-diff patterns
  - `slash` spec/parser ideas

## 2. Policy / Guard Layer

- **Canonical repo**: `hooks`
- **Owns**: pre/post tool guards, enforcement, safety, session lifecycle hooks
- **Must absorb**:
  - `ctx` destructive-command guard logic and shellcheck/logging patterns
  - useful `threesheets` freshness/context guard ideas
- **Must not become**: internal runtime orchestration logic that belongs in `looprs`

## 3. Tool Exposure Layer

- **Canonical repo**: `personal-mcp`
- **Owns**: MCP-exposed tools and stable tool contracts
- **Must absorb**:
  - `mcp-joecc` adapter/sync orchestration patterns
  - useful sync/operator ideas from work vault tooling when generalized
- **Must not become**: another orchestrator

## 4. Observability Pipeline

- **Canonical repos**:
  - `devloop` for runtime/operator observability
  - `message-extractor` for conversation normalization
  - `obfsck` for redaction
  - `inflection` for indexing/audit
- **Required unification**:
  - one shared event interchange boundary
  - optional redaction via `obfsck`
  - clear separation between normalization, redaction, indexing, and operator UI
- **Pattern donors**:
  - `peeprs`
  - `roxy`
  - `tbh`

## 5. Task / Context Layer

- **Canonical repos**:
  - `doob`
  - `kan`
- **Owns**: local task state, kanban view, agent-consumable context
- **Pattern donors**:
  - `joecc` data model ideas
  - `threesheets` bounded context persistence

## 6. Bootstrap / Environment Layer

- **Canonical repos**:
  - `dotfiles`
  - `pj`
- **Owns**: machine bootstrap, installation, runtime provisioning, operator entrypoint
- **Must absorb**:
  - `shell-aliases` and other stray infra fragments
- **Must decide**:
  - whether `pj` stays tightly personal or becomes a more reusable front door

## 7. Component / Content Layer

- **Canonical donor repo**: `steve`
- **Owns today**: component inventory only
- **Long-term role**: source of agents, commands, skills, templates for `looprs`, not an endlessly growing destination

# What Should Stay Separate

- `ai-vault` stays a vault-native orchestrator, not the universal platform
- `maestro` stays a work platform and pattern donor only
- `cwc` may stay standalone unless its indexing core clearly belongs in a shared library
- `obfsck` should remain a dependency-grade utility, not be folded into a larger repo

# What Gets Archived

Archive after extraction/reference capture:
- `goder`
- `gaw`
- `ctx`
- `joecc`
- `odk`
- `lisa`
- `code-mode-python`
- `mcp-joecc`
- `peeprs`
- `roxy`
- `tbh`
- low-value/empty repos already identified in `Consolidation Strategy`

# Project-By-Project Extraction Priorities

## Highest Value

- `looprs` -> runtime loop, session/event flow, command/skill/rule wiring
- `hooks` -> guard contract, deny-message UX, hook lifecycle shape
- `personal-mcp` -> MCP tool boundary and stable tool registration
- `devloop` -> trace/replay operator UX
- `message-extractor` -> transcript normalization pipeline
- `obfsck` -> reusable redaction library boundary
- `inflection` -> indexing/audit event model
- `doob` + `kan` -> task/state import and agent-consumable context surface
- `dotfiles` + `pj` -> bootstrap and operator front door
- `neocode` -> deterministic patch/validate loop, small-diff discipline, compaction
- `joecc` -> planner/executor split, sandbox posture
- `goder` -> provider routing and event propagation lessons
- `ctx` -> destructive-command guards and shell-quality gates
- `threesheets` -> freshness windows and rotating snapshots
- `gaw` -> local search/index primitives if needed
- `lisa` -> tiny-classifier routing and syntax-aware extraction
- `code-mode-python` -> execution substrate abstraction
- `mcp-joecc` -> adapter/sync orchestration ideas
- `maestro` -> generic runtime isolation and local orchestration patterns only

## Already Strong and Should Be Compounded

- `looprs` runtime core
- `hooks` production hook contract
- `personal-mcp` tool surface
- `obfsck` redaction utility
- `devloop` observability direction
- `message-extractor` normalization layer
- `inflection` audit/index layer
- `dotfiles` bootstrap substrate
- `pj` operator control plane

# Build Order

## Phase 1: Finish The Core Program Boundary

1. Wire agents and rules in `looprs`
2. Keep extracting `ctx` into `hooks`
3. Add contract tests and surface discipline to `personal-mcp`
4. Decide `pj` vs `dotfiles` boundary explicitly

## Phase 2: Unify Observability

5. Define the shared event schema across `devloop`, `message-extractor`, and `inflection`
6. Replace hand-rolled redaction with `obfsck` where applicable
7. Decide Ratatui vs Dioxus in `devloop`

## Phase 3: Extract Reference-Tier Gold

8. Pull `neocode` deterministic loop and compaction ideas into `looprs`
9. Pull `joecc` planner/sandbox ideas into the runtime/task layers
10. Pull `goder` routing/EventBus ideas into platform notes or code
11. Pull `threesheets` freshness/context patterns into hooks/context systems

## Phase 4: Reduce Sprawl

12. Strip down `steve` into a real donor repo
13. Resolve `gaw` vs `cwc` indexing future
14. Archive drained reference repos aggressively

# Implementation Mode

The canonical implementation path is now tracked in [Braid - Rust Workspace Blueprint](./Braid%20-%20Rust%20Workspace%20Blueprint.md).

# The Program In One Sentence

**A local-first, hook-governed, MCP-powered Braid platform with strong runtime boundaries, reproducible bootstrap, and structured observability.**

# Criticisms / Improvement Areas

- The biggest historical problem was not lack of good ideas; it was allowing too many sibling repos to behave like partial future centers of gravity at the same time.
- If this note is not used as a real boundary document, it will just become another accurate summary of sprawl.
- The hardest part is not implementation. It is refusing to build a new umbrella monolith and instead extracting the right ideas into the right canonical repos.

# Links

- `Consolidation Strategy`
- [Braid - Rust Workspace Blueprint](./Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Braid - Crate Implementation Checklist](./Braid%20-%20Crate%20Implementation%20Checklist.md)
- `Active Repos - Deep Dive Index`
- `Active Project Review Follow-up Tasks`
- `Reference Tier Review Follow-up Tasks`
- `Architecture Overlap Analysis`
