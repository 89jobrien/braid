# braid-hooks

`braid-hooks` adds ordered pre-execution policy checks and post-execution notifications around any
`braid_ports::ToolExecutor`.

## Workspace role

Hooks are external policy adapters rather than logic embedded in `braid-engine`. `HookedExecutor<T>`
wraps a concrete or registry-backed executor, evaluates every registered hook before dispatch, and
notifies all hooks after successful execution.

## Key APIs

| API | Purpose |
| --- | --- |
| `Hook`, `HookContext`, `HookVerdict` | Re-exported hook contract from `braid-ports` |
| `HookRegistry::new` | Create a fail-open registry |
| `HookRegistry::fail_closed` | Create a registry that denies when hook evaluation errors |
| `HookRegistry::register` | Add a hook in evaluation order |
| `HookedExecutor::new` | Wrap a tool executor with hooks and a session ID |
| `DestructiveCommandGuard` | Block configured destructive command substrings |
| `FreshnessGuard` | Placeholder freshness hook that currently always allows |

The first `Deny` verdict wins. In fail-open mode, hook errors are ignored; in fail-closed mode they
become a denial naming the failed hook. `post_execute` runs only after the inner executor succeeds.

## Usage

```rust
use braid_engine::StaticTool;
use braid_hooks::{DestructiveCommandGuard, HookRegistry, HookedExecutor};
use braid_model::{SessionId, ToolCall};
use braid_ports::ToolExecutor;

let hooks = HookRegistry::fail_closed().register(DestructiveCommandGuard::new());
let executor = HookedExecutor::new(
    StaticTool::new("shell", "completed"),
    hooks,
    SessionId("example".into()),
);

let denied = executor.execute(ToolCall {
    id: "call-1".into(),
    name: "shell".into(),
    input: "rm -rf /important".into(),
});
assert!(denied.is_err());
```

The destructive guard normalizes case and whitespace before checking patterns such as `rm -rf`, SQL
drop/truncate statements, force pushes, hard resets, `chmod 777`, and a fork bomb signature. Its
matching is substring-based, not a shell or SQL parser.

## Implementation status

Registry evaluation, fail-open/fail-closed behavior, executor wrapping, post notifications, and the
destructive guard are implemented. `FreshnessGuard` stores `max_age` but does not inspect context or
timestamps yet; it is explicitly a placeholder and always returns `Allow`.

## Features

This crate has no Cargo feature flags.

## Development and testing

```console
cargo nextest run -p braid-hooks
cargo clippy -p braid-hooks -- -D warnings
cargo check -p braid-hooks
```

Tests cover registry ordering, error policy, blocked command variants, successful pass-through, and
post-execution notification through cross-crate CLI integration tests.
