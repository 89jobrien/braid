# Model Hardening, Message Types, OpenAI Provider — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden braid-model with serde tests, add structured message/transcript types, and implement an OpenAI chat completions provider.

**Architecture:** Add `Message`, `ContentPart`, `Role`, `TokenCount`, `Transcript` to braid-model as the canonical conversation types. Update `ProviderRequest`/`ProviderResponse`/`RunInput`/`RunOutput` to use them. Add `OpenAiProvider` in braid-providers using reqwest blocking.

**Tech Stack:** Rust 1.88, edition 2024, serde_json, reqwest (blocking + json)

**Spec:** `docs/superpowers/specs/2026-03-14-model-transcript-openai-design.md`

---

## Chunk 1: Serde Tests for Existing Types

### Task 1: Add serde_json workspace dependency and write serde round-trip tests

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/braid-model/Cargo.toml`
- Create: `crates/braid-model/tests/serde_roundtrip.rs`

- [ ] **Step 1: Add serde_json to workspace and braid-model**

In root `Cargo.toml`, add to `[workspace.dependencies]`:
```toml
serde_json = "1"
```

In `crates/braid-model/Cargo.toml`, add to `[dependencies]`:
```toml
serde_json.workspace = true
```

And add `[dev-dependencies]`:
```toml
[dev-dependencies]
serde_json.workspace = true
```

Note: serde_json is needed as a regular dependency for `serde_json::Value` in `ContentPart::ToolUse` (Task 2), and as a dev-dependency for tests.

- [ ] **Step 2: Write serde round-trip tests for all existing public types**

Create `crates/braid-model/tests/serde_roundtrip.rs`:

```rust
use braid_model::*;
use serde_json;

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(*value, back);
}

#[test]
fn session_id_roundtrip() {
    roundtrip(&SessionId("sess-1".into()));
}

#[test]
fn session_state_roundtrip() {
    roundtrip(&SessionState::Planned);
    roundtrip(&SessionState::Running);
    roundtrip(&SessionState::WaitingForTool);
    roundtrip(&SessionState::Completed);
}

#[test]
fn tool_call_roundtrip() {
    roundtrip(&ToolCall {
        name: "echo".into(),
        input: "hello".into(),
    });
}

#[test]
fn tool_result_roundtrip() {
    roundtrip(&ToolResult {
        name: "echo".into(),
        output: "hello back".into(),
    });
}

#[test]
fn provider_request_roundtrip() {
    roundtrip(&ProviderRequest {
        prompt: "test prompt".into(),
    });
}

#[test]
fn provider_response_roundtrip() {
    roundtrip(&ProviderResponse {
        message: "test response".into(),
    });
}

#[test]
fn task_context_roundtrip() {
    roundtrip(&TaskContext {
        task_id: Some("task-1".into()),
        summary: "do stuff".into(),
    });
    roundtrip(&TaskContext {
        task_id: None,
        summary: "".into(),
    });
}

#[test]
fn event_roundtrip() {
    let sid = SessionId("s1".into());
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::SessionStarted,
    });
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::ProviderResponded,
    });
    roundtrip(&Event {
        session_id: sid.clone(),
        kind: EventKind::ToolCalled {
            tool_name: "echo".into(),
        },
    });
    roundtrip(&Event {
        session_id: sid,
        kind: EventKind::ToolCompleted {
            tool_name: "echo".into(),
        },
    });
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p braid-model`
Expected: All 7 tests pass.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/braid-model/Cargo.toml crates/braid-model/tests/serde_roundtrip.rs
git commit -m "test: add serde round-trip tests for all braid-model types"
```

---

## Chunk 2: Message and Transcript Types

### Task 2: Add new message types to braid-model

**Files:**
- Create: `crates/braid-model/src/message.rs`
- Create: `crates/braid-model/src/transcript.rs`
- Modify: `crates/braid-model/src/lib.rs`

- [ ] **Step 1: Create message.rs with Role, ContentPart, Message**

Create `crates/braid-model/src/message.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentPart {
    Text { text: String },
    Image { media_type: String, data: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
}
```

- [ ] **Step 2: Create transcript.rs with TokenCount and Transcript**

Create `crates/braid-model/src/transcript.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::session::SessionId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

- [ ] **Step 3: Wire up lib.rs exports**

Add to `crates/braid-model/src/lib.rs`:

```rust
pub mod message;
pub mod transcript;

pub use message::{ContentPart, Message, Role};
pub use transcript::{TokenCount, Transcript};
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p braid-model`
Expected: Compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-model/src/message.rs crates/braid-model/src/transcript.rs crates/braid-model/src/lib.rs
git commit -m "feat: add Message, ContentPart, Role, TokenCount, Transcript to braid-model"
```

### Task 3: Update ProviderRequest/ProviderResponse to use Message types

**Files:**
- Modify: `crates/braid-model/src/provider.rs`
- Modify: `crates/braid-core/src/engine.rs`
- Modify: `crates/braid-providers/src/lib.rs`
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Update provider.rs**

Replace contents of `crates/braid-model/src/provider.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::transcript::TokenCount;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderResponse {
    pub message: Message,
    pub token_count: Option<TokenCount>,
}
```

- [ ] **Step 2: Update RunInput and RunOutput in engine.rs**

Replace `RunInput` and `RunOutput` in `crates/braid-core/src/engine.rs`:

```rust
use braid_model::{Event, EventKind, Message, ProviderRequest, ProviderResponse, SessionId};

#[derive(Debug, Clone)]
pub struct RunInput {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub provider_response: ProviderResponse,
    pub events: Vec<Event>,
}
```

Update `Engine::run`:

```rust
impl<T, P> Engine<T, P>
where
    T: ToolExecutor,
    P: Provider,
{
    pub fn run(&self, input: RunInput) -> Result<RunOutput> {
        let provider_response = self.provider.complete(ProviderRequest {
            messages: input.messages,
        })?;
        let events = vec![
            Event {
                session_id: input.session_id.clone(),
                kind: EventKind::SessionStarted,
            },
            Event {
                session_id: input.session_id,
                kind: EventKind::ProviderResponded,
            },
        ];

        Ok(RunOutput {
            provider_response,
            events,
        })
    }
}
```

Remove the `use crate::tools::ToolExecutor;` import and `ToolCall`/`ToolResult` imports that are no longer needed in `run`. Keep the `ToolExecutor` import since it's still part of Engine's type param (retained for future use).

Update the test in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{ContentPart, Role};

    struct TestProvider;

    impl Provider for TestProvider {
        fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
            let first_text = request.messages.iter()
                .flat_map(|m| &m.content)
                .find_map(|c| match c {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            Ok(ProviderResponse {
                message: Message {
                    role: Role::Assistant,
                    content: vec![ContentPart::Text {
                        text: format!("provider saw: {}", first_text),
                    }],
                },
                token_count: None,
            })
        }
    }

    #[test]
    fn runs_a_minimal_session() {
        let engine = Engine::new(
            crate::tools::StaticTool::new("echo", "tool output"),
            TestProvider,
        );
        let output = engine
            .run(RunInput {
                session_id: SessionId("session-1".into()),
                messages: vec![Message {
                    role: Role::User,
                    content: vec![ContentPart::Text {
                        text: "hello".into(),
                    }],
                }],
            })
            .unwrap();

        let response_text = match &output.provider_response.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(response_text, "provider saw: hello");
        assert_eq!(output.events.len(), 2);
    }
}
```

- [ ] **Step 3: Update MockProvider in braid-providers**

Replace `crates/braid-providers/src/lib.rs`:

```rust
use anyhow::Result;
use braid_core::engine::Provider;
use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role};

#[derive(Debug, Default, Clone)]
pub struct MockProvider;

impl Provider for MockProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let first_text = request.messages.iter()
            .flat_map(|m| &m.content)
            .find_map(|c| match c {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Ok(ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text {
                    text: format!("mock response to: {}", first_text),
                }],
            },
            token_count: None,
        })
    }
}
```

- [ ] **Step 4: Update CLI main.rs**

Replace `crates/braid-cli/src/main.rs`:

```rust
use anyhow::Result;
use braid_core::{Engine, RunInput, StaticTool};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::MockProvider;

fn main() -> Result<()> {
    let engine = Engine::new(StaticTool::new("echo", "tool output"), MockProvider);
    let output = engine.run(RunInput {
        session_id: SessionId("demo-session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "hello from braid".into(),
            }],
        }],
    })?;

    let response_text = match &output.provider_response.message.content[0] {
        ContentPart::Text { text } => text.clone(),
        _ => "non-text response".into(),
    };
    println!("provider: {}", response_text);
    println!("events: {}", output.events.len());
    Ok(())
}
```

- [ ] **Step 5: Update serde round-trip tests for changed and new types**

In `crates/braid-model/tests/serde_roundtrip.rs`, replace the `provider_request_roundtrip` and `provider_response_roundtrip` tests, and add new tests for `Role`, `ContentPart`, `Message`, `TokenCount`, `Transcript`:

```rust
// Replace existing provider tests:

#[test]
fn provider_request_roundtrip() {
    roundtrip(&ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: "hello".into() }],
        }],
    });
}

#[test]
fn provider_response_roundtrip() {
    roundtrip(&ProviderResponse {
        message: Message {
            role: Role::Assistant,
            content: vec![ContentPart::Text { text: "hi".into() }],
        },
        token_count: None,
    });
    roundtrip(&ProviderResponse {
        message: Message {
            role: Role::Assistant,
            content: vec![ContentPart::Text { text: "hi".into() }],
        },
        token_count: Some(TokenCount { input: 10, output: 5 }),
    });
}

// Add new tests:

#[test]
fn role_roundtrip() {
    roundtrip(&Role::System);
    roundtrip(&Role::User);
    roundtrip(&Role::Assistant);
    roundtrip(&Role::Tool);
}

#[test]
fn content_part_roundtrip() {
    roundtrip(&ContentPart::Text { text: "hello".into() });
    roundtrip(&ContentPart::Image {
        media_type: "image/png".into(),
        data: "base64data".into(),
    });
    roundtrip(&ContentPart::ToolUse {
        id: "call_1".into(),
        name: "echo".into(),
        input: serde_json::json!({"key": "value"}),
    });
    roundtrip(&ContentPart::ToolResult {
        tool_use_id: "call_1".into(),
        content: "result".into(),
    });
}

#[test]
fn message_roundtrip() {
    roundtrip(&Message {
        role: Role::User,
        content: vec![
            ContentPart::Text { text: "look at this".into() },
            ContentPart::Image {
                media_type: "image/png".into(),
                data: "abc123".into(),
            },
        ],
    });
}

#[test]
fn token_count_roundtrip() {
    roundtrip(&TokenCount { input: 100, output: 50 });
}

#[test]
fn transcript_roundtrip() {
    roundtrip(&Transcript {
        session_id: SessionId("s1".into()),
        messages: vec![
            Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: "hi".into() }],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text { text: "hello".into() }],
            },
        ],
        token_count: Some(TokenCount { input: 5, output: 3 }),
    });
}
```

- [ ] **Step 6: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass. `cargo run -p braid-cli` prints the mock response.

- [ ] **Step 7: Commit**

```bash
git add crates/braid-model/src/provider.rs crates/braid-core/src/engine.rs crates/braid-providers/src/lib.rs crates/braid-cli/src/main.rs crates/braid-model/tests/serde_roundtrip.rs
git commit -m "refactor: update Provider types to use structured Message instead of plain strings"
```

---

## Chunk 3: OpenAI Provider

### Task 5: Add OpenAiProvider to braid-providers

**Files:**
- Modify: `crates/braid-providers/Cargo.toml`
- Create: `crates/braid-providers/src/openai.rs`
- Modify: `crates/braid-providers/src/lib.rs`

- [ ] **Step 1: Add dependencies to braid-providers**

Add to `crates/braid-providers/Cargo.toml` under `[dependencies]`:
```toml
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde_json.workspace = true
```

- [ ] **Step 2: Create openai.rs**

Create `crates/braid-providers/src/openai.rs`:

```rust
use anyhow::{bail, Context, Result};
use braid_core::engine::Provider;
use braid_model::{
    ContentPart, Message, ProviderRequest, ProviderResponse, Role, TokenCount,
};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OpenAiProvider {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.into(),
            client: reqwest::blocking::Client::new(),
        })
    }

    pub fn default_model() -> Result<Self> {
        Self::new("gpt-4o")
    }

    fn to_openai_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages.iter().map(|msg| self.to_openai_message(msg)).collect()
    }

    fn to_openai_message(&self, msg: &Message) -> Value {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        // Check for tool_call_id (Tool role messages)
        if msg.role == Role::Tool {
            if let Some(ContentPart::ToolResult { tool_use_id, content }) = msg.content.first() {
                return json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                });
            }
        }

        // Check for tool_calls (Assistant messages with ToolUse parts)
        let tool_calls: Vec<Value> = msg.content.iter().filter_map(|part| {
            match part {
                ContentPart::ToolUse { id, name, input } => Some(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": input.to_string(),
                    }
                })),
                _ => None,
            }
        }).collect();

        // Build content array from non-tool parts
        let content_parts: Vec<Value> = msg.content.iter().filter_map(|part| {
            match part {
                ContentPart::Text { text } => Some(json!({
                    "type": "text",
                    "text": text,
                })),
                ContentPart::Image { media_type, data } => Some(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", media_type, data),
                    }
                })),
                ContentPart::ToolUse { .. } => None,
                ContentPart::ToolResult { .. } => None,
            }
        }).collect();

        let mut msg_json = json!({ "role": role });

        if !tool_calls.is_empty() {
            msg_json["tool_calls"] = json!(tool_calls);
        }

        // Use string content if it's a single text part, array otherwise
        if content_parts.len() == 1 && content_parts[0].get("type") == Some(&json!("text")) {
            msg_json["content"] = content_parts[0]["text"].clone();
        } else if !content_parts.is_empty() {
            msg_json["content"] = json!(content_parts);
        }

        msg_json
    }

    fn parse_response(&self, body: Value) -> Result<ProviderResponse> {
        let choices = body["choices"]
            .as_array()
            .context("response missing choices array")?;
        if choices.is_empty() {
            bail!("response has empty choices array");
        }

        let choice_msg = &choices[0]["message"];
        let message = self.parse_message(choice_msg)?;

        let token_count = body.get("usage").and_then(|usage| {
            let input = usage.get("prompt_tokens")?.as_u64()?;
            let output = usage.get("completion_tokens")?.as_u64()?;
            Some(TokenCount { input, output })
        });

        Ok(ProviderResponse {
            message,
            token_count,
        })
    }

    fn parse_message(&self, msg: &Value) -> Result<Message> {
        let role_str = msg["role"]
            .as_str()
            .context("message missing role")?;
        let role = match role_str {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            other => bail!("unknown role: {}", other),
        };

        let mut content = Vec::new();

        // Parse text content
        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            content.push(ContentPart::Text { text: text.to_string() });
        }

        // Parse tool_calls
        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let func = &tc["function"];
                let name = func["name"].as_str().unwrap_or_default().to_string();
                let arguments_str = func["arguments"].as_str().unwrap_or("{}");
                let input: serde_json::Value = serde_json::from_str(arguments_str)
                    .unwrap_or(json!({}));
                content.push(ContentPart::ToolUse { id, name, input });
            }
        }

        Ok(Message { role, content })
    }
}

impl Provider for OpenAiProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let openai_messages = self.to_openai_messages(&request.messages);

        let body = json!({
            "model": self.model,
            "messages": openai_messages,
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .context("failed to send request to OpenAI")?;

        let status = response.status();
        let response_body: Value = response
            .json()
            .context("failed to parse OpenAI response as JSON")?;

        if !status.is_success() {
            bail!(
                "OpenAI API error ({}): {}",
                status,
                response_body
            );
        }

        self.parse_response(response_body)
    }
}
```

- [ ] **Step 3: Export OpenAiProvider from lib.rs**

Add to `crates/braid-providers/src/lib.rs`:

```rust
pub mod openai;
pub use openai::OpenAiProvider;
```

Keep the existing `MockProvider` code in `lib.rs`.

- [ ] **Step 4: Run cargo check**

Run: `cargo check --workspace`
Expected: Compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-providers/Cargo.toml crates/braid-providers/src/openai.rs crates/braid-providers/src/lib.rs
git commit -m "feat: add OpenAiProvider using chat completions API"
```

### Task 6: Wire OpenAI provider into CLI

**Files:**
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Update CLI to select provider via env var**

Replace `crates/braid-cli/src/main.rs`:

```rust
use anyhow::Result;
use braid_core::engine::Provider;
use braid_core::{Engine, RunInput, StaticTool};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::{MockProvider, OpenAiProvider};

fn main() -> Result<()> {
    let provider: Box<dyn Provider> = if std::env::var("OPENAI_API_KEY").is_ok() {
        Box::new(OpenAiProvider::default_model()?)
    } else {
        Box::new(MockProvider)
    };

    let engine = Engine::new(StaticTool::new("echo", "tool output"), provider);
    let output = engine.run(RunInput {
        session_id: SessionId("demo-session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "Say hello in one sentence.".into(),
            }],
        }],
    })?;

    let response_text = match &output.provider_response.message.content[0] {
        ContentPart::Text { text } => text.clone(),
        _ => "non-text response".into(),
    };
    println!("response: {}", response_text);
    if let Some(tc) = &output.provider_response.token_count {
        println!("tokens: {} in, {} out", tc.input, tc.output);
    }
    println!("events: {}", output.events.len());
    Ok(())
}
```

- [ ] **Step 2: Make Provider trait object-safe**

The `Provider` trait needs to work with `Box<dyn Provider>`. Check that `Engine` can accept `Box<dyn Provider>`. The current generic `Engine<T, P>` works if `P = Box<dyn Provider>` since `Box<dyn Provider>` implements `Provider` if we add a blanket impl. Alternatively, simplify by implementing `Provider` for `Box<dyn Provider>`:

In `crates/braid-core/src/engine.rs`, add after the `Provider` trait:

```rust
impl Provider for Box<dyn Provider> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}
```

- [ ] **Step 3: Run cargo check and test**

Run: `cargo check --workspace && cargo test --workspace`
Expected: Compiles and all tests pass.

- [ ] **Step 4: Test with mock (no API key)**

Run: `cargo run -p braid-cli`
Expected: Prints mock response.

- [ ] **Step 5: Commit**

```bash
git add crates/braid-cli/src/main.rs crates/braid-core/src/engine.rs
git commit -m "feat: wire OpenAI provider into CLI with env-var-based selection"
```
