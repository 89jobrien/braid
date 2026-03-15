# Provider Contract Tests + Ollama CI

## Goal

Replace MockProvider with real provider testing against Ollama (local) and OpenAI (optional). Run contract tests in CI against `qwen2.5:3b` on jobrien-vm.

## Architecture

The `Provider` trait is tested through a shared contract test suite that validates any implementation against three categories: text completion, tool calling, and error handling. Ollama support comes via a configurable base URL on `OpenAiProvider`, not a separate struct — Ollama's chat completions API is OpenAI-compatible.

## Changes

### 1. Remove MockProvider

- Delete `MockProvider` from `braid-providers/src/lib.rs`
- Remove from `braid-cli/src/main.rs` (resolve_provider, doctor)
- Remove from `provider_contract.rs`
- Remove `pub use` in `braid-providers/src/lib.rs`

### 2. Add ToolDefinition to braid-model

New type in `braid-model/src/tool.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Add `tools` field to `ProviderRequest`:

```rust
pub struct ProviderRequest {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}
```

The `#[serde(default)]` attribute ensures backward compatibility — existing serialized `ProviderRequest` JSON without a `tools` key will deserialize with an empty vec. All existing callers that construct `ProviderRequest` must add `tools: vec![]`.

### 3. Make OpenAiProvider base-URL configurable

Add `base_url: String` field:

```rust
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::blocking::Client,
}
```

Constructors:

- `OpenAiProvider::new(model)` — reads `OPENAI_API_KEY`, base URL `https://api.openai.com/v1`
- `OpenAiProvider::with_base_url(base_url, model, api_key)` — explicit, for any OpenAI-compatible API
- `OpenAiProvider::ollama(model)` — base URL `http://localhost:11434/v1`, empty API key
- `OpenAiProvider::default_model()` — unchanged, calls `new("gpt-4o")`

The `complete()` method posts to `{base_url}/chat/completions`.

### 4. Tool serialization in OpenAiProvider

When `request.tools` is non-empty, serialize into the OpenAI tools format:

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get current weather",
        "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
      }
    }
  ]
}
```

Only include `"tools"` key in the request body when the vec is non-empty.

### 5. Contract test suite

File: `crates/braid-providers/tests/provider_contract.rs`

Three contract checks:

**`verify_text_completion(provider)`**
- Send single user message "Say hello."
- Assert response role is Assistant
- Assert response contains at least one non-empty Text part

**`verify_tool_calling(provider)`**
- Define a `ToolDefinition` for `get_weather` with a `city` string parameter
- Send user message "What's the weather in Paris?" with the tool definition
- Assert response contains a `ToolUse` content part with non-empty id, name == "get_weather", and valid JSON input
- Build follow-up messages: original user message, assistant response with tool use, tool result message
- Send follow-up to provider
- Assert final response contains a Text part (the model's summary of the tool result)

**`verify_error_on_empty_messages(provider)`**
- Send `ProviderRequest` with empty messages vec
- Assert result is `Err` — validate this client-side in the provider adapter (before making the HTTP call) to avoid depending on provider-specific API error behavior

Two test functions — both `#[ignore]` so `cargo test` works on any dev machine without Ollama or API keys:

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
    let provider = OpenAiProvider::default_model().expect("OPENAI_API_KEY must be set");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
    verify_error_on_empty_messages(&provider).unwrap();
}
```

CI runs `cargo test --workspace --include-ignored` to execute all tests including the Ollama contract tests.

### 6. Install Ollama on jobrien-vm

- Install Ollama via official install script
- Pull `qwen2.5:3b`
- Ollama runs as systemd service (the installer sets this up)
- Verify with `curl http://localhost:11434/v1/models`

### 7. Update CI workflow

Note: `ubuntu-latest` is mapped to `ubuntu-latest:host` which runs on jobrien-vm (the act_runner host). Ollama is installed on the same machine, so localhost:11434 is accessible.

```yaml
- name: Check Ollama
  run: |
    curl -sf http://localhost:11434/api/tags || { echo "Ollama not running"; exit 1; }

- name: Test
  run: |
    source ~/.cargo/env
    cargo test --workspace --include-ignored
```

The `--include-ignored` flag runs both normal tests and `#[ignore]`-annotated contract tests. Contract tests hit Ollama at localhost:11434 — no env var needed (that's the default). The OpenAI contract test will still be skipped in CI (no `OPENAI_API_KEY` set), since `--include-ignored` runs ignored tests but the test itself will fail on the missing env var and that's handled by the `expect()` call — actually, we need to gate the OpenAI test on the env var being present:

```rust
#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    }
    // ...
}
```

This way `--include-ignored` won't fail on the OpenAI test when the key isn't available.

**Timeout:** Configure `reqwest::blocking::Client` with a 30-second timeout in all constructors to prevent hung Ollama from blocking CI indefinitely.

### 8. Update CLI

Remove mock provider fallback. New auto-detection order in `resolve_provider`:

1. If `--provider openai` flag: use OpenAI (requires `OPENAI_API_KEY`)
2. If `--provider ollama` flag: use Ollama at localhost:11434
3. If no flag: check `OPENAI_API_KEY` → OpenAI; else try Ollama at localhost:11434; else error

Remove doctor's mock-related code. Add Ollama connectivity check to doctor.

### 9. Serde round-trip tests

Already complete in `crates/braid-model/tests/serde_roundtrip.rs`. Add one test for the new `ToolDefinition` type.

## Files touched

| File | Action |
|------|--------|
| `crates/braid-model/src/tool.rs` | Add `ToolDefinition` |
| `crates/braid-model/src/lib.rs` | Re-export `ToolDefinition` |
| `crates/braid-model/src/provider.rs` | Add `tools` field to `ProviderRequest` |
| `crates/braid-model/tests/serde_roundtrip.rs` | Add `ToolDefinition` roundtrip test |
| `crates/braid-providers/src/lib.rs` | Remove `MockProvider` |
| `crates/braid-providers/src/openai.rs` | Add `base_url`, `with_base_url()`, `ollama()`, tool serialization |
| `crates/braid-providers/tests/provider_contract.rs` | Rewrite with full contract suite |
| `crates/braid-core/src/engine.rs` | Update `ProviderRequest` construction (add `tools: vec![]`) |
| `crates/braid-cli/src/main.rs` | Remove mock, add ollama provider, update doctor |
| `.gitea/workflows/ci.yml` | Add Ollama check step |

## Out of scope

- Streaming/async provider support
- Multiple tool calls in a single response
- Provider-specific configuration beyond base URL
- Ollama model management CLI commands
