# braid-model

`braid-model` is the canonical domain model for the Braid agent platform. It owns serializable
messages, provider requests and responses, tools, events, sessions, transcripts, tasks, and context
snapshots without depending on another Braid crate.

## Workspace role

This is the leaf of the workspace dependency graph. Higher layers exchange these types across port,
engine, provider, storage, context, MCP, and UI boundaries. Keeping the model independent prevents
adapters from leaking into the domain representation.

## Key APIs

| API | Purpose |
| --- | --- |
| `Message`, `Role`, `ContentPart` | Text, image, tool-use, and tool-result conversation data |
| `ProviderRequest`, `ProviderResponse` | Provider-neutral completion contract |
| `ToolDefinition`, `ToolCall`, `ToolResult` | Tool schema, invocation, and result values |
| `Event`, `EventKind` | Normalized session lifecycle and tool events |
| `SessionId`, `SessionPhase` | Session identity and state |
| `Transcript`, `TokenCount` | Persistable conversation and token accounting |
| `ContextChunk`, `ContextSummary`, `ContextSnapshot` | Captured and compacted context |
| `TaskContext` | Optional task identifier and summary |
| `estimate_tokens` | Rough UTF-8 byte-length token estimate (`len / 4`) |

All exported domain types derive `Serialize` and `Deserialize`. Many additionally derive equality,
clone, debug, hash, or default traits according to their fields.

## Usage

```rust
use braid_model::{ContentPart, Message, ProviderRequest, Role};

let request = ProviderRequest {
    messages: vec![Message {
        role: Role::User,
        content: vec![ContentPart::Text {
            text: "Say hello".into(),
        }],
    }],
    tools: vec![],
};

assert_eq!(request.messages.len(), 1);
```

Context can be rendered as markdown for injection into a provider request:

```rust
use braid_model::{ContextChunk, ContextSnapshot};

let chunk = ContextChunk::new("repo", "recent commits", "291bf5b docs(site): publish roadmap");
let snapshot = ContextSnapshot {
    token_estimate: chunk.token_estimate,
    chunks: vec![chunk],
    summary: None,
    assembled_at: chrono::Utc::now(),
    dropped_chunks: 0,
};

assert!(snapshot.render().contains("recent commits"));
```

## Implementation status

The model is implemented and used throughout the workspace. `EventKind::Unknown` provides an
explicit representation for unnormalized data, while storage readers may skip JSONL lines that do
not deserialize. The token estimator is intentionally approximate rather than tokenizer-specific.

## Features

This crate has no Cargo feature flags.

## Development and testing

Serialization compatibility is covered by `tests/serde_roundtrip.rs`; context rendering and token
accounting have unit tests in `src/context.rs`.

```console
cargo nextest run -p braid-model
cargo clippy -p braid-model -- -D warnings
cargo check -p braid-model
```
