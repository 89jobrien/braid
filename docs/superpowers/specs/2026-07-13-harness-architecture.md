# Braid as Harness: Architecture Spec

**Date:** 2026-07-13
**Status:** Draft
**Tracking:** HANDOFF — `Implement: harness-architecture`

---

## Framing

The OpenAI harness engineering post makes a claim worth internalising: **the harness is the product,
not the agent.** The agent is a commodity input. What compounds in value over time is the
environment — the tools, feedback signals, knowledge structure, and enforcement mechanisms the
agent operates inside.

Braid exists to be that environment. The current codebase (Phases 1–2) established the vertical
slice: provider → engine → tools → observe. This spec defines the next layer — what makes braid a
genuine harness rather than a thin agent runner.

The reframe: **the TUI chat window is a steering interface for a feedback loop, not a chatbot.**
The human specifies intent; the harness runs the agent; the agent validates its own output; the
harness surfaces the result. The conversation is between the human and the loop.

---

## What the harness must do

Five capabilities are missing from the current architecture:

### 1. Feedback loops inside the engine

The current `SimpleLoopPlanner` executes `CallProvider → ExecuteTool → Finish`. There is no
concept of a validation gate or self-review pass between turns. The harness planner needs phases:

```
Clarify → Execute → Validate → Review → (loop or Finish)
```

A `HarnessPlanner` replaces `SimpleLoopPlanner` as the default for harness sessions. It injects
a validation step after each significant tool execution (e.g. file write, shell command) and a
review pass before reporting completion. The engine's `Action` enum gains `Validate` and `Review`
variants. The planner controls when to loop vs. escalate to the human.

### 2. Repository knowledge as structured context

`RepoSource` currently feeds a diff stream. `DoobSource` feeds task list JSON. Neither gives the
agent the structured knowledge it needs to reason about intent, architecture, or quality.

New context sources:

| Source | What it fetches |
|---|---|
| `DocsSource` | Indexes `docs/` markdown — design docs, ADRs, quality grades |
| `PlanSource` | Reads active execution plans from `docs/superpowers/plans/` |
| `QualitySource` | Reads a quality score document tracking domain health |

The `ContextAssembler` already supports multiple sources and budget-gated summarisation. These
plug straight in.

`DocsSource` should index by recency and relevance-to-prompt, not exhaustively. The principle from
the article applies: give the agent a map, not a manual.

### 3. Agent-to-agent review

The harness needs a `Reviewer` role that runs a second agent against the first agent's output,
producing structured feedback the first agent can act on.

`braid-review` crate:

```
ReviewRequest { session_id, output, criteria: Vec<ReviewCriterion> }
ReviewResult  { findings: Vec<Finding>, verdict: Verdict }

trait Reviewer {
    fn review(&self, req: ReviewRequest) -> Result<ReviewResult>;
}
```

`AgentReviewer` runs a provider completion with the diff/output in context and a structured
output prompt. The engine's `HarnessPlanner` calls the reviewer after the `Validate` phase.
Multiple reviewers can be composed (correctness, style, security).

This is the Ralph Wiggum Loop encoded as a first-class planner phase.

### 4. Worktree isolation per task

Each harness session should operate in an isolated git worktree so concurrent sessions don't
conflict and the agent can freely stage/commit without polluting the working tree.

`braid-worktree` crate (thin wrapper over `git worktree`):

```
WorktreeGuard::create(repo_root, branch_name) -> Result<WorktreeGuard>
// Drops: removes the worktree and optionally the branch
```

The `Engine` gains an optional `WorktreeGuard` field. When set, all shell tool calls are scoped
to the worktree path via an env var or working directory injection through `HookedExecutor`.

On macOS this can use the existing repo. On minibox / Linux this enables true parallel agent
sessions.

### 5. Recurring background agents (garbage collection)

The article's "garbage collection" cadence — daily cleanup agents that enforce golden principles
and open fix-up PRs — requires a scheduler.

`braid-schedule` crate:

```
ScheduledTask { id, cron: &str, prompt: String, max_turns: Option<u32> }
Scheduler::run(tasks: Vec<ScheduledTask>, store: Arc<SessionStore>)
```

The scheduler is a long-running process (separate binary or `braid schedule` subcommand) that
fires tasks on their cron, runs them through the standard `Engine` pipeline, and writes results
to the session store. The TUI inspect pane can then browse scheduled task sessions.

Initial scheduled tasks:

- `doc-gardening`: scan `docs/` for stale content, open fix-up items
- `quality-audit`: regrade domain quality scores, write updated `docs/QUALITY.md`
- `debt-sweep`: scan for patterns violating golden principles, flag in doob

---

## Crate roadmap

```
Phase 3 (harness core):
  braid-review      — agent-to-agent review coordination
  braid-worktree    — per-task git worktree isolation
  braid-planner     — HarnessPlanner replacing SimpleLoopPlanner

Phase 4 (autonomy layer):
  braid-schedule    — recurring background agent tasks
  braid-context     — extend with DocsSource, PlanSource, QualitySource
  braid-cli         — braid schedule subcommand, braid review subcommand
```

`braid-planner` may be a rename/replacement of the planner trait in `braid-engine`, or a new
crate that depends on it. Decision deferred to implementation.

---

## Dependency graph (updated)

```
braid-cli
  → braid-providers
  → braid-review       (new)
  → braid-worktree     (new)
  → braid-schedule     (new)
  → braid-engine
      → braid-hooks
      → braid-redact
      → braid-context
          → braid-ports
          → braid-model
      → braid-observe
      → braid-model
```

---

## What braid does NOT do

Braid is not:

- A UI framework (the TUI is a thin steering layer, not the product)
- A CI/CD system (it fires agents; pipelines are external)
- A code review platform (it coordinates agent review; humans opt in)
- A model fine-tuning pipeline

Braid is the environment that makes agents effective at a specific repository. Its value is the
feedback loops, knowledge structure, and enforcement mechanisms it provides — not the model
behind the provider.

---

## Golden principles (initial set)

These are the "braid way" invariants the garbage collection agents will enforce:

1. **Context is a map, not a manual.** No source may inject more than 2000 tokens of context
   without summarisation. `ContextAssembler` budget enforcement is the gate.

2. **Redact before persist.** The `RedactionPipeline` runs on all provider responses before
   they are written to the session store. This is already enforced in `Engine::run`.

3. **Hooks are policy, not logic.** `HookedExecutor` enforces external rules (destructive
   command guard, freshness). Business logic lives in the planner.

4. **One session, one worktree.** Harness sessions run in isolated worktrees. The engine does
   not write to the caller's working tree.

5. **All knowledge in the repo.** Context sources may only read from the local filesystem or
   well-defined CLI tools (doob, git). No external API calls in context assembly.

---

## Reference

- OpenAI harness engineering post (2026-02-10) — primary inspiration
- `docs/planning/Braid - Rust Workspace Spec.md` — crate dependency rules
- `docs/planning/Braid - Crate Implementation Checklist.md` — phase tracking
- `crates/braid-engine/src/lib.rs` — current Engine/Planner/Action types
- `crates/braid-context/src/` — existing context assembly pipeline
