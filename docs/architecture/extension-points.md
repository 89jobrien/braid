# Extension Points

Braid is designed around hexagonal architecture. The engine core is pure Rust with no I/O — all external concerns attach via traits.

## The Three Core Traits

### `Provider` — LLM Backend

```rust
pub trait Provider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}
```

Implement this to add a new LLM backend. The engine calls it with `ProviderRequest { messages, tools }` and expects a `ProviderResponse { message, token_count }`.

Existing implementations: `OpenAiProvider` (also handles Ollama via compatible API), `MockProvider` (for tests).

### `ToolExecutor` — Tool Dispatch

```rust
pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}
```

The engine calls this when the planner returns `Action::ExecuteTool`. The call includes `{ id, name, input: String }` (JSON string from the LLM).

Existing implementations: `ToolRegistry` (dispatches by name to registered executors), `StaticTool` (always returns a fixed string), `HookedExecutor<T>` (wraps any executor with policy gates).

### `Planner` — Loop Strategy

```rust
pub trait Planner {
    fn next_action(&self, state: &SessionState) -> Result<Action>;
}
```

Decides what the engine does next given the current session state. Returns `Action::CallProvider`, `Action::ExecuteTool`, or `Action::Finish`.

Existing implementation: `SimpleLoopPlanner` (default tool-call loop, max 20 turns).

## Engine Builder

```
Engine::new(tool_executor, provider)         // required
    .with_redactor(|msg| ...)                // optional: transform messages before provider
    .with_event_callback(|event| ...)        // optional: react to events synchronously
    .run(input, &planner)
```

Both `.with_redactor` and `.with_event_callback` accept any `Fn` with `Send + Sync + 'static` bounds, enabling closures that capture `Arc<Mutex<T>>` state.

## Redaction

```rust
pub trait RedactionRule: Send + Sync {
    fn name(&self) -> &str;
    fn redact(&self, input: &str) -> String;
}
```

Implement to add a new redaction pattern. Wire into the pipeline:

```rust
RedactionPipeline::new()
    .with_rule(SecretPatternRule::new())
    .with_rule(MyCustomRule::new())
```

Rules apply in order. The first rule's output is the second rule's input.

Built-in rules: `SecretPatternRule`, `EnvVarRule`, `HomePathRule`.

## Hooks

```rust
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    fn post_execute(&self, ctx: &HookContext, result: &ToolResult) {}  // optional
}

pub enum HookVerdict {
    Allow,
    Deny { reason: String, remediation: String },
}
```

Implement to add pre/post execution policy. Wire via `HookedExecutor`:

```rust
let registry = HookRegistry::new()
    .register(DestructiveCommandGuard::new())
    .register(MyCustomHook::new());

let hooked = HookedExecutor::new(ToolRegistry::new(), registry, session_id);
let engine = Engine::new(hooked, provider);
```

`HookRegistry::fail_closed()` makes hook errors deny instead of allow.

Built-in hooks: `DestructiveCommandGuard` (blocks `rm -rf`, `DROP TABLE`, force push, etc.), `FreshnessGuard` (placeholder).

## Ingestion

```rust
pub trait Ingester {
    fn ingest(&self, source: &Path, store: &SessionStore) -> Result<SessionId>;
}
```

Implement to normalize a new external session format into braid events. The ingester reads the source file, maps its format to `Vec<Event>`, and calls `store.write()`.

Built-in ingesters: `BraidIngester` (native JSONL pass-through), `ClaudeCodeIngester` (Claude Code conversation logs), `DevloopIngester` (devloop run transcripts).

## Wiring Diagram

```
             ┌─────────────────────────────┐
             │        Engine<T, P>         │
             │                             │
  ┌──────────┤  T: ToolExecutor            │
  │          │  P: Provider                │
  │          │  redactor: Option<Fn>       │
  │          │  event_callback: Option<Fn> │
  │          └──────────┬──────────────────┘
  │                     │
  │           ┌─────────┘
  │           │
  ▼           ▼
┌──────────────────┐    ┌──────────────────┐
│ HookedExecutor   │    │ OpenAiProvider   │
│  inner: T        │    │  (or custom)     │
│  registry:       │    └──────────────────┘
│   HookRegistry   │
│    ├─ Guard1     │
│    └─ Guard2     │
│  └─ ToolRegistry │
│      ├─ tool_a   │
│      └─ tool_b   │
└──────────────────┘

event_callback closure captures:
  Arc<Mutex<Option<SessionWriter>>>
  RedactionPipeline

redactor closure captures:
  RedactionPipeline
```

## Adding a New Provider

1. Create a struct in `braid-providers`
2. Implement `Provider` trait
3. Wire in `braid-cli`'s `resolve_provider()` function

No changes to `braid-core`, `braid-model`, or any other crate.

## Adding a New Tool

1. Implement `ToolExecutor` (or use `StaticTool` for simple cases)
2. Register: `registry.register("my_tool", Box::new(MyTool::new()))`
3. Optionally expose via MCP: `mcp_registry.register(ToolDefinition { name: "my_tool", ... })`

## Adding a New CLI Subcommand

1. Add a variant to the `clap`-derived `Commands` enum in `braid-cli/src/main.rs`
2. Implement a `cmd_*` function that delegates to library crates
3. Match the variant in `main()`

The CLI is intentionally thin — no business logic lives there.
