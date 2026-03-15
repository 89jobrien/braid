# Provider Contract Tests + Ollama CI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace MockProvider with real provider testing against Ollama, add tool definition support, and run contract tests in CI.

**Architecture:** Make `OpenAiProvider` base-URL configurable so it works with any OpenAI-compatible API (including Ollama). Add `ToolDefinition` to `braid-model` and wire tool definitions through `ProviderRequest`. Contract tests validate text completion, tool calling, and error handling against real providers. Ollama runs on jobrien-vm alongside the CI runner.

**Tech Stack:** Rust, reqwest (blocking), serde_json, Ollama, Gitea Actions

**Spec:** `docs/superpowers/specs/2026-03-15-provider-contract-tests-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/braid-model/src/tool.rs` | Add `ToolDefinition` struct |
| `crates/braid-model/src/lib.rs` | Re-export `ToolDefinition` |
| `crates/braid-model/src/provider.rs` | Add `tools` field to `ProviderRequest` |
| `crates/braid-model/tests/serde_roundtrip.rs` | Add `ToolDefinition` roundtrip + updated `ProviderRequest` test |
| `crates/braid-providers/src/openai.rs` | Add `base_url` field, new constructors, tool serialization, empty-message validation, 30s timeout |
| `crates/braid-providers/src/lib.rs` | Remove `MockProvider`, re-export only `OpenAiProvider` |
| `crates/braid-providers/tests/provider_contract.rs` | Full contract test suite |
| `crates/braid-core/src/engine.rs` | Update `ProviderRequest` construction sites (add `tools: vec![]`) |
| `crates/braid-cli/src/main.rs` | Remove mock, add ollama, update doctor |
| `.gitea/workflows/ci.yml` | Add Ollama check, use `--include-ignored` |

---

## Chunk 1: Model Layer Changes

### Task 1: Add ToolDefinition to braid-model

**Files:**
- Modify: `crates/braid-model/src/tool.rs`
- Modify: `crates/braid-model/src/lib.rs`
- Test: `crates/braid-model/tests/serde_roundtrip.rs`

- [ ] **Step 1: Write the failing test for ToolDefinition serde roundtrip**

Add to `crates/braid-model/tests/serde_roundtrip.rs`:

```rust
#[test]
fn tool_definition_roundtrip() {
    roundtrip(&ToolDefinition {
        name: "get_weather".into(),
        description: "Get current weather for a city".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"]
        }),
    });
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p braid-model tool_definition_roundtrip`
Expected: FAIL — `ToolDefinition` not found / cannot resolve

- [ ] **Step 3: Implement ToolDefinition**

Add to `crates/braid-model/src/tool.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Add to `crates/braid-model/src/lib.rs` re-exports:

```rust
pub use tool::{ToolCall, ToolDefinition, ToolResult};
```

Add `use serde_json;` import in `tool.rs` if not present (it's not — `serde_json` is in Cargo.toml but not imported in tool.rs).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p braid-model tool_definition_roundtrip`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/braid-model/src/tool.rs crates/braid-model/src/lib.rs crates/braid-model/tests/serde_roundtrip.rs
git commit -m "feat(model): add ToolDefinition type with serde roundtrip test"
```

---

### Task 2: Add tools field to ProviderRequest

**Depends on:** Task 1 (ToolDefinition must exist and be re-exported)

**Files:**
- Modify: `crates/braid-model/src/provider.rs`
- Modify: `crates/braid-model/tests/serde_roundtrip.rs`
- Modify: `crates/braid-core/src/engine.rs` (update construction sites)
- Modify: `crates/braid-providers/tests/provider_contract.rs` (update construction site)
- Modify: `crates/braid-cli/src/main.rs` (update construction site in doctor)

- [ ] **Step 1: Write the failing test — ProviderRequest with tools roundtrip**

Update the existing `provider_request_roundtrip` test in `crates/braid-model/tests/serde_roundtrip.rs` to include the `tools` field:

```rust
#[test]
fn provider_request_roundtrip() {
    // Without tools
    roundtrip(&ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "hello".into(),
            }],
        }],
        tools: vec![],
    });
    // With tools
    roundtrip(&ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "hello".into(),
            }],
        }],
        tools: vec![ToolDefinition {
            name: "get_weather".into(),
            description: "Get weather".into(),
            parameters: serde_json::json!({"type": "object"}),
        }],
    });
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p braid-model provider_request_roundtrip`
Expected: FAIL — `ProviderRequest` has no field `tools`

- [ ] **Step 3: Add tools field to ProviderRequest**

Modify `crates/braid-model/src/provider.rs`:

```rust
use crate::message::Message;
use crate::tool::ToolDefinition;
use crate::transcript::TokenCount;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequest {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}
```

- [ ] **Step 4: Fix all compilation errors**

Every site that constructs `ProviderRequest` must add `tools: vec![]`. These are:

**`crates/braid-core/src/engine.rs:102`** — in `Engine::run` match arm:
```rust
let response = self.provider.complete(ProviderRequest { messages, tools: vec![] })?;
```

**`crates/braid-core/src/engine.rs` tests** — `TestProvider::complete`, `ToolCallingProvider::complete`, `InfiniteToolProvider::complete`, `NeverProvider::complete` all receive `ProviderRequest` but don't construct it, so no change needed there.

**`crates/braid-providers/tests/provider_contract.rs`** — the existing `verify_provider_contract` function at line 7:
```rust
let request = ProviderRequest {
    messages: vec![Message {
        role: Role::User,
        content: vec![ContentPart::Text {
            text: "Say hello.".into(),
        }],
    }],
    tools: vec![],
};
```

**`crates/braid-cli/src/main.rs`** — two sites:
1. `cmd_run` at line 82 (inside `RunInput` — this passes `messages` to `Engine::run` which constructs `ProviderRequest` internally, so no change here)
2. `doctor::check_openai_connectivity` at line 169:
```rust
let request = ProviderRequest {
    messages: vec![Message {
        role: Role::User,
        content: vec![ContentPart::Text { text: "hi".into() }],
    }],
    tools: vec![],
};
```

- [ ] **Step 5: Run all tests to verify everything passes**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/braid-model/src/provider.rs crates/braid-model/tests/serde_roundtrip.rs crates/braid-core/src/engine.rs crates/braid-providers/tests/provider_contract.rs crates/braid-cli/src/main.rs
git commit -m "feat(model): add tools field to ProviderRequest"
```

---

## Chunk 2: OpenAiProvider Refactor

### Task 3: Add base_url and timeout to OpenAiProvider

**Files:**
- Modify: `crates/braid-providers/src/openai.rs`

- [ ] **Step 1: Write the failing test — ollama constructor creates provider**

Add a test module at the bottom of `crates/braid-providers/src/openai.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_constructor_sets_base_url() {
        let provider = OpenAiProvider::ollama("qwen2.5:3b");
        assert_eq!(provider.base_url, "http://localhost:11434/v1");
        assert_eq!(provider.model, "qwen2.5:3b");
    }

    #[test]
    fn with_base_url_constructor() {
        let provider = OpenAiProvider::with_base_url(
            "http://custom:8080/v1",
            "my-model",
            "my-key",
        );
        assert_eq!(provider.base_url, "http://custom:8080/v1");
        assert_eq!(provider.model, "my-model");
        assert_eq!(provider.api_key, "my-key");
    }

    #[test]
    fn rejects_empty_messages() {
        let provider = OpenAiProvider::ollama("qwen2.5:3b");
        let request = ProviderRequest {
            messages: vec![],
            tools: vec![],
        };
        let err = provider.complete(request).unwrap_err();
        assert!(err.to_string().contains("empty"), "expected empty messages error, got: {err}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p braid-providers -- tests::ollama_constructor`
Expected: FAIL — no method `ollama` / no field `base_url`

- [ ] **Step 3: Implement base_url, constructors, and empty-message validation**

Modify `crates/braid-providers/src/openai.rs`:

Add `base_url` field to struct:
```rust
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::blocking::Client,
}
```

Build a client with 30s timeout (place this as a free function above the `impl OpenAiProvider` block):
```rust
fn build_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}
```

Update existing constructors and add new ones:
```rust
impl OpenAiProvider {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.into(),
            base_url: "https://api.openai.com/v1".into(),
            client: build_client(),
        })
    }

    pub fn default_model() -> Result<Self> {
        Self::new("gpt-4o")
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
            client: build_client(),
        }
    }

    pub fn ollama(model: impl Into<String>) -> Self {
        Self::with_base_url("http://localhost:11434/v1", model, "ollama")
    }
    // ... rest unchanged
}
```

Add empty-message validation at the top of `complete()`:
```rust
fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
    if request.messages.is_empty() {
        bail!("cannot complete with empty messages");
    }
    // ... rest of method
}
```

Update the hardcoded URL in `complete()` to use `self.base_url`:
```rust
let response = self
    .client
    .post(format!("{}/chat/completions", self.base_url))
    .header("Authorization", format!("Bearer {}", self.api_key))
    .json(&body)
    .send()
    .context("failed to send request")?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p braid-providers`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/braid-providers/src/openai.rs
git commit -m "feat(providers): add base_url, ollama constructor, empty-message validation"
```

---

### Task 4: Add tool definition serialization to OpenAiProvider

**Files:**
- Modify: `crates/braid-providers/src/openai.rs`

- [ ] **Step 1: Write the failing test — tool definitions serialized in request body**

Add to the test module in `crates/braid-providers/src/openai.rs`:

```rust
#[test]
fn serializes_tool_definitions_in_request_body() {
    use braid_model::ToolDefinition;

    let provider = OpenAiProvider::ollama("test-model");
    let tools = vec![ToolDefinition {
        name: "get_weather".into(),
        description: "Get weather for a city".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }),
    }];

    let body = provider.build_request_body(
        &[Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: "test".into() }],
        }],
        &tools,
    );

    let tools_json = body["tools"].as_array().expect("tools should be an array");
    assert_eq!(tools_json.len(), 1);
    assert_eq!(tools_json[0]["type"], "function");
    assert_eq!(tools_json[0]["function"]["name"], "get_weather");
}

#[test]
fn omits_tools_key_when_empty() {
    let provider = OpenAiProvider::ollama("test-model");
    let body = provider.build_request_body(
        &[Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: "test".into() }],
        }],
        &[],
    );

    assert!(body.get("tools").is_none(), "tools key should be absent when empty");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p braid-providers -- serializes_tool`
Expected: FAIL — no method `build_request_body`

- [ ] **Step 3: Extract build_request_body and add tool serialization**

Add a new method to `OpenAiProvider`:

```rust
fn build_request_body(&self, messages: &[Message], tools: &[braid_model::ToolDefinition]) -> Value {
    let openai_messages = self.to_openai_messages(messages);
    let mut body = json!({
        "model": self.model,
        "messages": openai_messages,
    });

    if !tools.is_empty() {
        let tools_json: Vec<Value> = tools
            .iter()
            .map(|t| json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            }))
            .collect();
        body["tools"] = json!(tools_json);
    }

    body
}
```

Update `complete()` to use it:
```rust
fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
    if request.messages.is_empty() {
        bail!("cannot complete with empty messages");
    }

    let body = self.build_request_body(&request.messages, &request.tools);

    let response = self
        .client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key))
        .json(&body)
        .send()
        .context("failed to send request")?;
    // ... rest unchanged
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p braid-providers`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/braid-providers/src/openai.rs
git commit -m "feat(providers): serialize tool definitions in OpenAI request body"
```

---

## Chunk 3: Remove MockProvider + Update CLI

### Task 5: Remove MockProvider and update CLI

**Files:**
- Modify: `crates/braid-providers/src/lib.rs`
- Modify: `crates/braid-cli/src/main.rs`

- [ ] **Step 1: Remove MockProvider from lib.rs**

Replace `crates/braid-providers/src/lib.rs` entirely:

```rust
pub mod openai;
pub use openai::OpenAiProvider;
```

- [ ] **Step 2: Update CLI — remove mock, add ollama**

In `crates/braid-cli/src/main.rs`:

Remove the `MockProvider` import:
```rust
// Change this:
use braid_providers::{MockProvider, OpenAiProvider};
// To this:
use braid_providers::OpenAiProvider;
```

Update `resolve_provider`:
```rust
fn resolve_provider(flag: Option<&str>, model: &str) -> Result<Box<dyn Provider>> {
    let provider_name = match flag {
        Some(name) => name.to_string(),
        None => {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                "openai".into()
            } else {
                "ollama".into()
            }
        }
    };

    match provider_name.as_str() {
        "ollama" => Ok(Box::new(OpenAiProvider::ollama(model))),
        "openai" => Ok(Box::new(OpenAiProvider::new(model)?)),
        other => bail!("unknown provider: {other} (expected 'ollama' or 'openai')"),
    }
}
```

Update the `Run` command's model default — when using Ollama, the default model should still make sense. Keep `gpt-4o` as the default since `resolve_provider` will select the right provider. Users can pass `--model qwen2.5:3b` for Ollama.

Update `doctor` module — replace mock/OpenAI-specific checks:

Remove `check_openai_connectivity` and replace with a more general approach:
```rust
mod doctor {
    use anyhow::Result;
    use std::process::Command as ProcessCommand;

    pub fn run_checks() -> Result<()> {
        check_rust_toolchain();
        check_openai_key();
        check_ollama_connectivity();
        check_openai_connectivity();
        check_workspace_health();
        Ok(())
    }

    fn check_rust_toolchain() {
        // ... unchanged
    }

    fn check_openai_key() {
        // ... unchanged
    }

    fn check_ollama_connectivity() {
        let output = ProcessCommand::new("curl")
            .args(["-sf", "http://localhost:11434/api/tags"])
            .output();
        match output {
            Ok(out) if out.status.success() => println!("ollama ... ok"),
            _ => println!("ollama ... not reachable (http://localhost:11434)"),
        }
    }

    fn check_openai_connectivity() {
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("openai connectivity ... skipped (no API key)");
            return;
        }

        use braid_core::Provider;
        use braid_model::{ContentPart, Message, ProviderRequest, Role};
        use braid_providers::OpenAiProvider;

        match OpenAiProvider::new("gpt-4o") {
            Ok(provider) => {
                let request = ProviderRequest {
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text { text: "hi".into() }],
                    }],
                    tools: vec![],
                };
                match provider.complete(request) {
                    Ok(_) => println!("openai connectivity ... ok"),
                    Err(e) => println!("openai connectivity ... FAIL ({e})"),
                }
            }
            Err(e) => println!("openai connectivity ... FAIL ({e})"),
        }
    }

    fn check_workspace_health() {
        // ... unchanged
    }
}
```

- [ ] **Step 3: Replace provider_contract.rs with stub (it references the deleted MockProvider)**

Replace `crates/braid-providers/tests/provider_contract.rs` with a minimal stub that uses the new `OpenAiProvider::ollama` constructor:

```rust
use anyhow::Result;
use braid_core::Provider;
use braid_model::{ContentPart, Message, ProviderRequest, Role};
use braid_providers::OpenAiProvider;

fn verify_text_completion(provider: &impl Provider) -> Result<()> {
    let request = ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "Say hello.".into(),
            }],
        }],
        tools: vec![],
    };

    let response = provider.complete(request)?;

    assert_eq!(
        response.message.role,
        Role::Assistant,
        "response role must be Assistant"
    );
    assert!(
        !response.message.content.is_empty(),
        "response content must not be empty"
    );

    let has_text = response
        .message
        .content
        .iter()
        .any(|part| matches!(part, ContentPart::Text { text } if !text.is_empty()));
    assert!(
        has_text,
        "response must contain at least one non-empty Text part"
    );

    Ok(())
}

#[test]
#[ignore = "requires Ollama running locally"]
fn ollama_provider_satisfies_contract() {
    let provider = OpenAiProvider::ollama("qwen2.5:3b");
    verify_text_completion(&provider).unwrap();
}

#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    }
    let provider = OpenAiProvider::default_model().expect("OPENAI_API_KEY must be set");
    verify_text_completion(&provider).unwrap();
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --workspace`
Expected: All PASS (contract tests are `#[ignore]` so they won't run)

- [ ] **Step 5: Commit**

```bash
git add crates/braid-providers/src/lib.rs crates/braid-cli/src/main.rs crates/braid-providers/tests/provider_contract.rs
git commit -m "feat: remove MockProvider, add ollama support to CLI"
```

---

## Chunk 4: Full Contract Test Suite

### Task 6: Add tool-calling contract test

**Files:**
- Modify: `crates/braid-providers/tests/provider_contract.rs`

- [ ] **Step 1: Write verify_tool_calling function**

Add to `provider_contract.rs`:

```rust
use braid_model::ToolDefinition;

fn verify_tool_calling(provider: &impl Provider) -> Result<()> {
    let tool = ToolDefinition {
        name: "get_weather".into(),
        description: "Get the current weather for a given city".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["city"]
        }),
    };

    // Step 1: Send a message that should trigger the tool
    let request = ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "What is the current weather in Paris? Use the get_weather tool.".into(),
            }],
        }],
        tools: vec![tool.clone()],
    };

    let response = provider.complete(request)?;

    // Verify response contains a ToolUse part
    let tool_use = response
        .message
        .content
        .iter()
        .find_map(|part| match part {
            ContentPart::ToolUse { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
            _ => None,
        });

    let (tool_call_id, tool_name, _tool_input) =
        tool_use.expect("response must contain a ToolUse content part");

    assert!(!tool_call_id.is_empty(), "tool call id must not be empty");
    assert_eq!(tool_name, "get_weather", "tool name must match requested tool");

    // Step 2: Send the tool result back and get final response
    let follow_up = ProviderRequest {
        messages: vec![
            Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: "What is the current weather in Paris? Use the get_weather tool.".into(),
                }],
            },
            response.message.clone(),
            Message {
                role: Role::Tool,
                content: vec![ContentPart::ToolResult {
                    tool_use_id: tool_call_id,
                    content: "Sunny, 22°C".into(),
                }],
            },
        ],
        tools: vec![tool],
    };

    let final_response = provider.complete(follow_up)?;

    let has_text = final_response
        .message
        .content
        .iter()
        .any(|part| matches!(part, ContentPart::Text { text } if !text.is_empty()));
    assert!(
        has_text,
        "final response after tool result must contain text"
    );

    Ok(())
}
```

- [ ] **Step 2: Wire into test functions**

Update both test functions to call `verify_tool_calling`:

```rust
#[test]
#[ignore = "requires Ollama running locally"]
fn ollama_provider_satisfies_contract() {
    let provider = OpenAiProvider::ollama("qwen2.5:3b");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
}

#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    }
    let provider = OpenAiProvider::default_model().expect("OPENAI_API_KEY must be set");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo test -p braid-providers --no-run`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/braid-providers/tests/provider_contract.rs
git commit -m "feat(providers): add tool-calling contract test"
```

---

### Task 7: Add error-handling contract test

**Files:**
- Modify: `crates/braid-providers/tests/provider_contract.rs`

- [ ] **Step 1: Write verify_error_on_empty_messages function**

Add to `provider_contract.rs`:

```rust
fn verify_error_on_empty_messages(provider: &impl Provider) -> Result<()> {
    let request = ProviderRequest {
        messages: vec![],
        tools: vec![],
    };

    let result = provider.complete(request);
    assert!(
        result.is_err(),
        "provider must return Err for empty messages"
    );

    Ok(())
}
```

- [ ] **Step 2: Wire into test functions**

Update both test functions to also call `verify_error_on_empty_messages`:

```rust
#[test]
#[ignore = "requires Ollama running locally"]
fn ollama_provider_satisfies_contract() {
    let provider = OpenAiProvider::ollama("qwen2.5:3b");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
    verify_error_on_empty_messages(&provider).unwrap();
}

#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    }
    let provider = OpenAiProvider::default_model().expect("OPENAI_API_KEY must be set");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
    verify_error_on_empty_messages(&provider).unwrap();
}
```

- [ ] **Step 3: Run error test locally (this one doesn't need Ollama)**

The `verify_error_on_empty_messages` test validates client-side validation that happens before any HTTP call, so we can test it directly:

Run: `cargo test -p braid-providers -- --include-ignored ollama_provider_satisfies_contract 2>&1 | head -20`

This will fail on the Ollama connection for `verify_text_completion`, but we can verify the empty-messages validation works by temporarily testing just that function. Alternatively, add a dedicated non-ignored test:

```rust
#[test]
fn empty_messages_returns_error() {
    let provider = OpenAiProvider::ollama("any-model");
    let request = ProviderRequest {
        messages: vec![],
        tools: vec![],
    };
    assert!(provider.complete(request).is_err());
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p braid-providers empty_messages_returns_error`
Expected: PASS

- [ ] **Step 5: Run all workspace tests**

Run: `cargo test --workspace`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/braid-providers/tests/provider_contract.rs
git commit -m "feat(providers): add empty-messages error contract test"
```

---

## Chunk 5: Infrastructure — Ollama + CI

### Task 8: Install Ollama on jobrien-vm

**Files:** None (infrastructure)

SSH to jobrien-vm — use the `1password-tailscale` skill for credentials and SSH pattern. All commands below use `SSH_CMD` as shorthand for the full SSH invocation to `dev@100.105.75.7`.

- [ ] **Step 1: Install Ollama**

```bash
SSH_CMD dev@100.105.75.7 "curl -fsSL https://ollama.com/install.sh | sh"
```

- [ ] **Step 2: Verify Ollama is running**

```bash
SSH_CMD dev@100.105.75.7 "curl -sf http://localhost:11434/api/tags"
```

Expected: JSON response with `{"models": [...]}`

- [ ] **Step 3: Pull qwen2.5:3b**

```bash
SSH_CMD dev@100.105.75.7 "ollama pull qwen2.5:3b"
```

This will take a few minutes to download (~1.9GB).

- [ ] **Step 4: Verify model is available**

```bash
SSH_CMD dev@100.105.75.7 "ollama list | grep qwen"
```

Expected: Line showing `qwen2.5:3b`

- [ ] **Step 5: Quick smoke test**

```bash
SSH_CMD dev@100.105.75.7 "curl -s http://localhost:11434/v1/chat/completions -H 'Content-Type: application/json' -d '{\"model\":\"qwen2.5:3b\",\"messages\":[{\"role\":\"user\",\"content\":\"Say hi\"}]}' | head -c 200"
```

Expected: JSON response with a choices array containing an assistant message

---

### Task 9: Update CI workflow

**Files:**
- Modify: `.gitea/workflows/ci.yml`

- [ ] **Step 1: Update CI workflow**

Replace `.gitea/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: ['**']
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Check Ollama
        run: |
          curl -sf http://localhost:11434/api/tags || { echo "Ollama not running"; exit 1; }

      - name: Check formatting
        run: |
          source ~/.cargo/env
          cargo fmt --all -- --check

      - name: Clippy
        run: |
          source ~/.cargo/env
          cargo clippy --workspace -- -D warnings

      - name: Test
        run: |
          source ~/.cargo/env
          cargo test --workspace --include-ignored
```

- [ ] **Step 2: Commit and push**

```bash
git add .gitea/workflows/ci.yml
git commit -m "ci: add Ollama check, run ignored contract tests"
git push origin main
```

- [ ] **Step 3: Verify CI passes**

Check the Gitea CI run:
```bash
curl -s -u joe:braid-gitea "http://100.105.75.7:3000/api/v1/repos/joe/braid/actions/runs?limit=1" | python3 -c "import sys,json; runs=json.load(sys.stdin).get('workflow_runs',[]); r=runs[0] if runs else {}; print(f'Run #{r.get(\"id\",\"?\")} status={r.get(\"status\",\"?\")} conclusion={r.get(\"conclusion\",\"?\")}')"
```

Expected: `status=completed conclusion=success`

If it fails, check logs:
```bash
curl -s -u joe:braid-gitea "http://100.105.75.7:3000/api/v1/repos/joe/braid/actions/jobs/N/logs" 2>&1 | grep -E "(error|FAIL|Failure)" | head -20
```
