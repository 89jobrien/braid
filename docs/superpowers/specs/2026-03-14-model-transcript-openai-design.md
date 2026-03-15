# Design: Serde Tests, Message/Transcript Types, OpenAI Provider

**Date**: 2026-03-14
**Status**: Approved
**Scope**: Phase 1 completion — braid-model hardening, structured message types, first real provider

---

## 1. Serde Round-Trip Tests (braid-model)

Add serde JSON round-trip tests for every public type in `braid-model`: `Event`, `EventKind`, `SessionId`, `SessionState`, `ToolCall`, `ToolResult`, `ProviderRequest`, `ProviderResponse`, `TaskContext`.

Each test serializes to JSON and deserializes back, asserting equality.

---

## 2. Message and Transcript Types (braid-model)

### New Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentPart {
    Text { text: String },
    Image { media_type: String, data: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenCount {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub token_count: Option<TokenCount>,
}
```

### Changes to Existing Types

`ProviderRequest`: replace `prompt: String` with `messages: Vec<Message>`.

`ProviderResponse`: replace `message: String` with `message: Message`, add `token_count: Option<TokenCount>`.

### Changes to RunInput

`RunInput`: replace `prompt: String` with `messages: Vec<Message>`. Remove `tool: ToolCall` — tool calls come back in provider responses, not in run inputs. `RunInput` becomes `{ session_id, messages }`.

`RunOutput`: keep `provider_response` and `events`. Remove `tool_result` for now (tool execution will be wired in a later phase when the engine loop handles tool_calls from provider responses).

These changes propagate to `braid-core` (Engine, tests), `braid-providers` (MockProvider), and `braid-cli` (main).

### Dependencies

Add `serde_json = "1"` to `[workspace.dependencies]` in root `Cargo.toml`. `braid-model` uses it via `serde_json.workspace = true`.

---

## 3. OpenAI Provider (braid-providers)

### Adapter

`OpenAiProvider` implements the `Provider` trait using OpenAI's chat completions API (`/v1/chat/completions`).

- API key: read from `OPENAI_API_KEY` env var at construction time
- Model: configurable, defaults to `gpt-4o`
- HTTP: `reqwest` blocking client
- Maps `Vec<Message>` to OpenAI's message format and back

### Type Mapping

| Braid | OpenAI JSON |
|---|---|
| `Role::System` | `"system"` |
| `Role::User` | `"user"` |
| `Role::Assistant` | `"assistant"` |
| `Role::Tool` | `"tool"` |
| `ContentPart::Text` | `{"type": "text", "text": "..."}` |
| `ContentPart::Image` | `{"type": "image_url", "image_url": {"url": "data:..."}}` |
| `ContentPart::ToolUse` | `tool_calls[].{id, type: "function", function: {name, arguments}}` — `arguments` is a JSON string in OpenAI; parse to `serde_json::Value` for `input` |
| `ContentPart::ToolResult` | `"tool"` role message with `tool_call_id` |
| `ContentPart::Image` | `{"type": "image_url", "image_url": {"url": "data:{media_type};base64,{data}"}}` — `data` field is raw base64, construct the data URI at mapping time |

### Response Parsing

Extract the first choice's message, map it back to `Message`. If `choices` is empty, `anyhow::bail!`. Parse `usage.prompt_tokens` and `usage.completion_tokens` into `TokenCount` (both fields optional in response — return `None` if `usage` is absent).

### Dependencies

`braid-providers` gains: `reqwest` (with `blocking` and `json` features), `serde_json`.

### Error Handling

- Missing `OPENAI_API_KEY`: fail at provider construction with clear error
- HTTP/API errors: propagate via `anyhow`
- Unexpected response shape: `anyhow::bail!` with the raw body

---

## Build Order

1. Add `serde_json` to `braid-model`, add serde round-trip tests for existing types
2. Add `Role`, `ContentPart`, `Message`, `TokenCount`, `Transcript` to `braid-model`
3. Update `ProviderRequest`/`ProviderResponse`, propagate changes through core/providers/cli
4. Add serde round-trip tests for new types
5. Add `OpenAiProvider` to `braid-providers`
6. Wire up CLI to use OpenAI provider (with fallback to mock via flag or env)
