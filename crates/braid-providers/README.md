# braid-providers

`braid-providers` supplies synchronous LLM provider adapters for Braid. The current production
adapter targets OpenAI-compatible chat-completions APIs and can connect to OpenAI or local Ollama.

## Workspace role

This adapter crate implements `braid_ports::Provider` and translates between `braid-model` values
and OpenAI-compatible JSON. The engine depends on the port, not this crate; operator frontends
choose and construct a provider.

## Providers

### `OpenAiProvider`

- `OpenAiProvider::new(model)` reads and validates `OPENAI_API_KEY` and uses
  `https://api.openai.com/v1`.
- `OpenAiProvider::default_model()` selects `gpt-4o`.
- `OpenAiProvider::with_base_url(base_url, model, api_key)` targets another compatible endpoint.
- `OpenAiProvider::ollama(model)` targets `http://localhost:11434/v1` with the placeholder key
  `ollama`.

The blocking HTTP client has a 30-second timeout. It supports text, base64 image parts, function
tool definitions, assistant tool calls, tool results, and optional token usage. Empty message lists
are rejected before a network request.

```rust
use braid_providers::OpenAiProvider;

let local = OpenAiProvider::ollama("qwen2.5:3b");
let hosted = OpenAiProvider::new("gpt-4o")?;
# Ok::<(), anyhow::Error>(())
```

### `MockProvider`

The fixed-text `MockProvider` is available only with `test-support`:

```rust
use braid_providers::MockProvider;

let provider = MockProvider::with_text("hello from a test");
```

## Feature flags

| Feature | Effect |
| --- | --- |
| `test-support` | Exposes the `mock` module and `MockProvider` test double |

## Implementation status

OpenAI and Ollama-compatible chat completions are implemented through one adapter. The package
description mentions Ollama because `OpenAiProvider::ollama` configures its compatible endpoint;
there is no separate Ollama type. Streaming responses and provider-specific APIs are not present.
Malformed JSON tool arguments are logged and converted to an empty object.

## Development and testing

The normal test suite verifies request serialization and response parsing without network access.
Ignored contract tests require external services:

```console
cargo nextest run -p braid-providers
cargo nextest run -p braid-providers --features test-support
cargo test -p braid-providers --test provider_contract -- --ignored
```

The ignored OpenAI test requires `OPENAI_API_KEY`; the Ollama test requires a local Ollama server
and the `qwen2.5:3b` model.
