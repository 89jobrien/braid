# Braid Roadmap

This is the current delivery roadmap for Braid. GitHub issues hold executable scope and status;
the documents under `docs/planning/` preserve the architecture history and extraction rationale.

## Delivered Foundation

Braid currently provides:

- canonical domain types and hexagonal ports through `braid-model` and `braid-ports`;
- the generic `braid-engine` runtime with provider and tool execution boundaries;
- OpenAI-compatible providers, ordered redaction, pre/post hooks, and MCP tool exposure;
- session persistence, replay, rendering, and a Ratatui inspector;
- bounded context assembly from repository and task sources;
- setup and health checks plus manifest-backed commands and prompts; and
- CLI and TUI composition roots for local operation.

The authoritative crate list is in [`Cargo.toml`](./Cargo.toml). The delivered baseline reflects
the workspace at commit `52d57b5`; the earlier `braid-core` crate was renamed to `braid-engine`
in commit `6672edf`.

## Delivery Order

| Phase | Epic | Priority | Depends on |
| --- | --- | --- | --- |
| 1 | [Secure runtime and persistence boundaries](https://github.com/89jobrien/braid/issues/42) | P1 | - |
| 2 | [Add review and worktree isolation](https://github.com/89jobrien/braid/issues/43) | P2 | #42 |
| 3 | [Add structured repository knowledge](https://github.com/89jobrien/braid/issues/44) | P2 | #43 |
| 4 | [Schedule recurring maintenance agents](https://github.com/89jobrien/braid/issues/45) | P3 | #42, #43, #44 |

### Phase 1: Foundation Hardening

Tracking: [#42](https://github.com/89jobrien/braid/issues/42)

Make the existing runtime safe before increasing autonomy:

- enforce redaction at persistence boundaries;
- contain session and component paths beneath configured roots;
- isolate concurrent session buffers;
- bound MCP input allocation;
- advertise registered tools to providers; and
- align architecture checks, manifests, and parallel-test behavior.

Exit condition: the safety and contract regressions in #42 are covered by tests and the full
workspace gate passes.

### Phase 2: Harness Core

Tracking: [#43](https://github.com/89jobrien/braid/issues/43)

Move from a linear agent runner to a repository harness:

- add bounded execute, validate, review, and escalation phases;
- coordinate structured agent-to-agent review;
- isolate each harness session in a git worktree; and
- keep orchestration outside the CLI and TUI frontends.

Exit condition: concurrent harness sessions can validate and review their work without touching
the caller's working tree.

### Phase 3: Structured Repository Knowledge

Tracking: [#44](https://github.com/89jobrien/braid/issues/44)

Extend context assembly with a repository map rather than an exhaustive manual:

- add relevant and recent documentation context;
- expose active implementation plans and checked-in quality state;
- preserve source provenance and freshness; and
- enforce per-source and final snapshot budgets after summarization.

Exit condition: a harness session receives a bounded, source-backed view of repository intent,
active work, and quality constraints.

### Phase 4: Autonomy Layer

Tracking: [#45](https://github.com/89jobrien/braid/issues/45)

Run opt-in recurring maintenance through the standard engine pipeline:

- validate and schedule bounded agent tasks;
- support deterministic one-shot execution and long-running scheduling;
- reuse hooks, redaction, context, worktrees, and session persistence; and
- expose scheduled runs through existing CLI and TUI operator surfaces.

Exit condition: documentation gardening, quality auditing, and debt sweeping can run safely as
inspectable Braid sessions without replacing CI/CD.

## Roadmap Governance

- GitHub issue state is authoritative for future work and completion.
- This file records phase order, outcomes, and dependencies; implementation detail stays in the
  linked epic.
- Completed capabilities are not recreated as issues solely because an old checklist remains
  unchecked.
- Material scope changes require updating the linked issue and this file together.

## Historical Sources

- [Workspace blueprint](./docs/planning/Braid%20-%20Rust%20Workspace%20Blueprint.md)
- [Workspace specification](./docs/planning/Braid%20-%20Rust%20Workspace%20Spec.md)
- [Crate implementation checklist](./docs/planning/Braid%20-%20Crate%20Implementation%20Checklist.md)
- [Harness architecture draft](./docs/superpowers/specs/2026-07-13-harness-architecture.md)

## Non-Goals

Braid is not a CI/CD system, remote code-review platform, distributed scheduler, model training
pipeline, or general-purpose repository index. It remains a local-first environment for bounded,
observable, policy-governed agents.
