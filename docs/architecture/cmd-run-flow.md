# `braid run` End-to-End Data Flow

This document traces the complete path of a `braid run "say hello"` invocation from CLI entry to persisted session.

## Overview

```
CLI args
  │
  ▼
cmd_run()
  │   resolves provider + prompt
  │   builds two RedactionPipelines
  │   generates session_id (unix timestamp)
  │   opens SessionWriter
  │
  ▼
Engine::run()
  │   drives agent loop via SimpleLoopPlanner
  │   emits events → event_callback → redact → SessionWriter
  │
  ▼
OpenAiProvider::complete()
  │   HTTP POST to OpenAI/Ollama
  │
  ▼
RunOutput
  │   SessionWriter.finish() → meta.json
  │   print response to stdout
  └─  print token counts to stderr
```

## Step-by-Step

### 1. Provider Resolution

```
if OPENAI_API_KEY in env → OpenAiProvider::new(model)   (api.openai.com)
else                     → OpenAiProvider::ollama(model)  (localhost:11434)
```

`--provider` flag overrides. `--model` flag sets the model string (default: `gpt-4o`).

### 2. Prompt Resolution

```
if PROMPT arg provided → use it
else                   → read all of stdin until EOF
```

### 3. Session ID

Generated from the current Unix timestamp in seconds:

```rust
SessionId(format!("{}", SystemTime::now()...as_secs()))
```

This gives sessions a human-readable, monotonically increasing ID (e.g., `1743300000`).

### 4. Redaction Pipelines

Two pipelines are built with identical rules:

```
SecretPatternRule   →  redacts AWS keys, GitHub tokens, Bearer tokens, sk-* keys
EnvVarRule          →  redacts KEY=value where KEY looks like a secret name
HomePathRule        →  replaces /Users/joe/... with ~/...
```

**`msg_pipeline`** — wired via `Engine::with_redactor`. Applied to every `Message` before it is sent to the provider. Prevents secrets in prompts or tool results from reaching the LLM API.

**`event_pipeline`** — wired via `Engine::with_event_callback`. Applied to every `Event` before it is written to disk. Prevents secrets from reaching the session store.

### 5. SessionWriter Setup

```rust
let writer: Arc<Mutex<Option<SessionWriter>>> = Arc::new(Mutex::new(
    SessionWriter::open("~/.braid/sessions/", &session_id).ok()
));
```

Using `Arc<Mutex<Option<...>>>` because:
- `Arc` — shared ownership between the callback closure and the post-run finalization
- `Mutex` — the callback may run multiple times (once per event); needs exclusive access
- `Option` — graceful degradation if the store directory can't be opened

### 6. Engine Configuration

```
Engine::new(ToolRegistry::new(), provider)
    .with_redactor(|msg| msg_pipeline.redact_message(msg))
    .with_event_callback(|event| {
        let redacted = event_pipeline.redact_event(event);
        writer_cb.lock() → writer.write_event(&redacted)
    })
```

### 7. Engine Run

```
engine.run(RunInput {
    session_id: session_id.clone(),
    messages: [Message { role: User, content: [Text { text: prompt }] }],
    max_turns: None,
}, &SimpleLoopPlanner)
```

As the engine runs, each event fires the callback:

```
SessionStarted    → redact → write to events.jsonl (flushed immediately)
ProviderResponded → redact → write
ToolCalled        → redact → write
ToolCompleted     → redact → write
SessionCompleted  → redact → write
```

The session is crash-safe: events are on disk as they happen. Even if the process dies mid-session, partial events are readable (just no `meta.json`).

### 8. Session Finalization

```rust
writer.lock() → writer.take() → writer.finish()
```

`finish()` writes `meta.json` atomically via temp-file rename:

```
~/.braid/sessions/1743300000/
├── events.jsonl    ← streaming events (written during run)
└── meta.json       ← {session_id, written_at, event_count} (written after run)
```

### 9. Output

```
stdout: <response text>
stderr: tokens: 42 in, 18 out
```

## Full Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│ braid-cli: cmd_run()                                                │
│                                                                     │
│  prompt ──────────────────────────────────────────────────────┐    │
│                                                               │    │
│  RedactionPipeline (msg)       RedactionPipeline (event)      │    │
│         │                              │                      │    │
│         │                       SessionWriter                 │    │
│         │                              │                      │    │
│  ┌──────▼──────────────────────────────▼──────────────────┐  │    │
│  │ Engine<ToolRegistry, OpenAiProvider>                    │  │    │
│  │                                                         │  │    │
│  │  run(RunInput)                                          │  │    │
│  │   │                                                     │  │    │
│  │   ├─ emit SessionStarted ──────────────────────────────▶│write │
│  │   │                                                     │  │    │
│  │   │  ┌──────────────────────────────────────────────┐  │  │    │
│  │   │  │ SimpleLoopPlanner loop                       │  │  │    │
│  │   │  │                                              │  │  │    │
│  │   │  │  CallProvider:                               │  │  │    │
│  │   │  │   messages ──(redact)──▶ ProviderRequest     │  │  │    │
│  │   │  │                              │               │  │  │    │
│  │   │  │          ┌───────────────────┘               │  │  │    │
│  │   │  │          │ HTTP POST /chat/completions        │  │  │    │
│  │   │  │          ▼                                   │  │  │    │
│  │   │  │   OpenAI / Ollama API                        │  │  │    │
│  │   │  │          │                                   │  │  │    │
│  │   │  │          ▼ ProviderResponse                  │  │  │    │
│  │   │  │   emit ProviderResponded ──────────────────▶ │write │   │
│  │   │  │                                              │  │  │    │
│  │   │  │  ExecuteTool:                                │  │  │    │
│  │   │  │   emit ToolCalled ─────────────────────────▶ │write │   │
│  │   │  │   ToolRegistry::execute(ToolCall)            │  │  │    │
│  │   │  │   emit ToolCompleted ──────────────────────▶ │write │   │
│  │   │  │                                              │  │  │    │
│  │   │  └──────────────────────────────────────────────┘  │  │    │
│  │   │                                                     │  │    │
│  │   ├─ emit SessionCompleted ─────────────────────────────▶│write │
│  │   │                                                     │  │    │
│  │   └─ return RunOutput ──────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  writer.finish() → meta.json                                        │
│  println!(response)                                                 │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
              ~/.braid/sessions/{session_id}/
              ├── events.jsonl
              └── meta.json
```

## Error Handling

| Failure point | Behavior |
|---|---|
| Provider unreachable | `engine.run()` returns `Err`; session has partial events but no `meta.json` |
| Store dir unwritable | `SessionWriter::open` returns `None`; session runs normally, nothing persisted |
| `finish()` fails | Warning printed to stderr; session events are still on disk |
| Tool execution fails | Engine propagates error; `SessionCompleted` is NOT emitted |
