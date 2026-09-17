# Design: Roadmap Governance

## Goal

Create one current roadmap that reconciles historical planning documents with the implemented
workspace and tracks remaining work through four linked GitHub milestone epics.

## Approved Approach

Use milestone epics: foundation hardening, harness core, structured repository knowledge, and
the autonomy layer.

## Context Map

### Files to Modify

| File | Purpose | Change |
| --- | --- | --- |
| `ROADMAP.md` | Current delivery sequence | Add delivered baseline and four linked phases |
| `README.md` | Project entry point | Correct crate inventory and link the roadmap |

### Source Documents

| File | Relationship |
| --- | --- |
| `docs/planning/Braid - Crate Implementation Checklist.md` | Historical crate phases and hardening work |
| `docs/superpowers/specs/2026-07-13-harness-architecture.md` | Current harness and autonomy proposal |
| `Cargo.toml` | Authoritative workspace membership |
| `crates/braid-cli/src/main.rs` | Authoritative CLI surface and composition |

### Verification

Documentation links and issue URLs will be checked after generation. No Rust API,
serialization format, CLI output, or crate dependency changes are part of this work.

## Roadmap Structure

1. **Delivered foundation** records capabilities already present in the workspace.
2. **Foundation hardening** resolves known safety, persistence, and contract gaps.
3. **Harness core** introduces validation/review phases, reviewer coordination, and worktree
   isolation.
4. **Structured repository knowledge** adds bounded docs, plan, and quality context sources.
5. **Autonomy layer** schedules recurring maintenance agents and exposes operator controls.

Each future phase maps to one GitHub issue containing scope, source references, acceptance
criteria, dependencies, and explicit exclusions.

## GitHub Issue Contract

- Titles use `epic(<scope>): <outcome>` and remain under 70 characters.
- Issues use existing `enhancement` or `documentation` labels only.
- Issue bodies link the source planning documents and include checkable acceptance criteria.
- Later phases explicitly depend on earlier issue numbers.
- Existing open issues are checked before creation to prevent duplicates.

## README Integration

The crate list will use the current `braid-engine` name, include `braid-bootstrap` and
`braid-components`, and link `ROADMAP.md` before historical planning documents.

## Out of Scope

- Implementing any roadmap capability.
- Rewriting or deleting historical planning documents.
- Creating GitHub milestones, projects, or new labels.
- Changing Rust source, public APIs, manifests, or CI.

## Risk

- [ ] Breaking API changes: no.
- [ ] New external dependency: no.
- [ ] Feature flag required: no.
- [ ] Planning drift: mitigated by naming `ROADMAP.md` as the current status document and
      treating older plans as historical sources.
