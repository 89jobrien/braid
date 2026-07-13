# braid CLI — Operator Contract

This document describes the stable operator interface for the `braid` binary. It covers
subcommands, flags, environment variables, session storage, provider configuration, MCP
mode, agent harness mode, and exit codes. Only behaviour present in the source code is
documented here.

---

## Invocation

```
braid <SUBCOMMAND> [OPTIONS]
```

All subcommands are required. Running `braid` without a subcommand prints help and exits 2.

---

## Subcommands

### `braid run`

Run a single prompt against a provider and print the response to stdout.

```
braid run [PROMPT] [--provider <NAME>] [--model <MODEL>]
```

| Argument / Flag     | Default    | Description                                                        |
| ------------------- | ---------- | ------------------------------------------------------------------ |
| `PROMPT`            | (none)     | Prompt text. Reads from stdin if omitted and stdin is not a tty.   |
| `--provider <NAME>` | auto       | Provider to use: `openai` or `ollama`. See Provider Configuration. |
| `--model <MODEL>`   | `gpt-4o`   | Model name passed to the provider.                                 |

**Stdin**: if `PROMPT` is not provided and stdin is a terminal, the command errors. If stdin
is a pipe or redirect, the full contents are read as the prompt. An empty stdin prompt is
an error.

**Output**: the final text response is written to stdout. Token counts (`N in, M out`) are
written to stderr.

**Redaction**: the redaction pipeline (`SecretPatternRule`, `EnvVarRule`, `HomePathRule`)
runs on all messages before they are persisted to the session store.

---

### `braid agent`

Same engine pipeline as `run`, but emits every session event as a JSON line on stdout,
followed by a final `response` JSON line. Intended for warpx and editor integrations.

```
braid agent [--prompt <TEXT>] [--provider <NAME>] [--model <MODEL>] [--max-turns <N>]
```

| Flag                | Default    | Description                                                        |
| ------------------- | ---------- | ------------------------------------------------------------------ |
| `--prompt <TEXT>`   | (none)     | Prompt text. Reads from stdin if omitted and stdin is not a tty.   |
| `--provider <NAME>` | auto       | Provider to use: `openai` or `ollama`.                             |
| `--model <MODEL>`   | `gpt-4o`   | Model name passed to the provider.                                 |
| `--max-turns <N>`   | unlimited  | Maximum engine turns before the loop stops.                        |

**Session ID**: if the environment variable `OZ_RUN_ID` is set, its value is used as the
session ID. Otherwise a Unix timestamp (seconds) is generated.

**Output format**: after the engine completes, stored session events are read back and
each is serialised as a single JSON line. The final line is always a `response` object:

```jsonc
// Session lifecycle events (one per line):
{"session_id": "1720000000", "kind": "SessionStarted"}
{"session_id": "1720000000", "kind": "ProviderResponded"}
{"session_id": "1720000000", "kind": {"ToolCalled": {"tool_name": "refresh_context"}}}
{"session_id": "1720000000", "kind": {"ToolCompleted": {"tool_name": "refresh_context"}}}
{"session_id": "1720000000", "kind": "SessionCompleted"}

// Final response line (always last):
{"type": "response", "text": "...", "tokens": {"input": 120, "output": 45}}
```

The `tokens` field is `null` when the provider does not return token counts.

---

### `braid mcp`

Start an MCP (Model Context Protocol) server over stdio. See MCP Mode below.

```
braid mcp
```

No flags. Reads JSON-RPC messages from stdin, writes responses to stdout. Runs until EOF.

---

### `braid sessions`

Manage stored sessions.

```
braid sessions <ACTION>
```

#### `braid sessions list`

Print all session IDs to stdout, newest first (one ID per line). Prints `no sessions found`
when the store is empty.

#### `braid sessions show <ID>`

Print the event timeline for session `<ID>` to stdout in a human-readable format. Errors
if the session does not exist.

#### `braid sessions prune [--keep <N>]`

Delete the oldest sessions, keeping the `N` most recent. Prints `deleted <N> session(s)`.

| Flag          | Default | Description                      |
| ------------- | ------- | -------------------------------- |
| `--keep <N>`  | `50`    | Number of recent sessions to keep.|

---

### `braid doctor`

Run environment health checks and print a summary table. Checks performed:

- Rust toolchain present
- `git` on PATH
- `doob` on PATH
- `cargo-nextest` installed
- `cargo-deny` installed
- `OPENAI_API_KEY` set
- Ollama reachable (connectivity probe)
- OpenAI API reachable (connectivity probe)
- Workspace health
- `~/.braid/` config directory exists

Exits 0 regardless of individual check results.

---

### `braid setup`

Create the `~/.braid/` directory structure if it does not already exist.

```
braid setup
```

No flags. Errors if `HOME` is not set.

---

## Session Model

A **session** is a single `Engine::run` invocation, identified by a `SessionId` string. By
default the ID is the current Unix timestamp in seconds. In agent mode, `OZ_RUN_ID` overrides
this.

### Storage layout

Sessions are stored under `~/.braid/sessions/` (requires `HOME` to be set):

```
~/.braid/sessions/
  <session-id>/
    events.jsonl   # one JSON object per line, append-only
    meta.json      # written atomically after the session completes
```

`meta.json` is absent for sessions that did not complete normally (e.g. process crash). The
store tolerates a missing `meta.json` — `events.jsonl` is still readable.

### JSONL event format

Each line of `events.jsonl` is a JSON object with two fields:

```jsonc
{"session_id": "<id>", "kind": <EventKind>}
```

`EventKind` variants and their serialised forms:

| Variant              | JSON                                               |
| -------------------- | -------------------------------------------------- |
| `SessionStarted`     | `"SessionStarted"`                                 |
| `ProviderResponded`  | `"ProviderResponded"`                              |
| `ToolCalled`         | `{"ToolCalled": {"tool_name": "<name>"}}`          |
| `ToolCompleted`      | `{"ToolCompleted": {"tool_name": "<name>"}}`       |
| `SessionCompleted`   | `"SessionCompleted"`                               |
| `Unknown`            | `{"Unknown": {"raw": "<opaque string>"}}`          |

Lines that do not deserialise to a known `EventKind` are silently skipped when loading,
providing forward compatibility with future variants.

### `meta.json` format

```json
{
  "session_id": "1720000000",
  "written_at": "2026-07-12T10:30:00Z",
  "event_count": 5
}
```

`written_at` is UTC, RFC 3339, second precision. `meta.json` is written atomically via a
temp-file rename — a partial `meta.json` is never visible to concurrent readers.

---

## Provider Configuration

### Auto-detection

When `--provider` is not specified:

1. If `OPENAI_API_KEY` is set in the environment, `openai` is selected.
2. Otherwise `ollama` is selected.

### OpenAI

```
OPENAI_API_KEY=sk-... braid run "hello"
braid run "hello" --provider openai --model gpt-4o-mini
```

`OPENAI_API_KEY` must be set. The provider uses the standard OpenAI chat completions
endpoint. If `OPENAI_API_KEY` is absent when `openai` is selected (explicitly or by
auto-detect after key is removed), the provider construction fails and the command exits 1.

### Ollama

```
braid run "hello" --provider ollama --model llama3.2
```

No API key required. The Ollama provider uses the local Ollama server (default address used
by `OpenAiProvider::ollama`). `--model` is the Ollama model name to pass.

### Context summarization

In `run` and `agent` modes, a secondary `OpenAiProvider` instance is created for context
summarization. If `OPENAI_API_KEY` is not set, summarization is disabled and a note is
printed to stderr (`note: no provider available for context summarization`). The primary
provider and session are not affected.

---

## MCP Mode

`braid mcp` starts a Model Context Protocol server on stdio using the MCP 2024-11-05
protocol. It is suitable for wiring braid as a tool server to any MCP-compliant client
(e.g. Claude Desktop, warpx, other LLM agents).

### Transport

Content-Length framed JSON-RPC 2.0 over stdin/stdout:

```
Content-Length: <N>\r\n
\r\n
<N bytes of JSON>
```

Responses use the same framing. The server runs until stdin reaches EOF.

### Protocol version

```json
{"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, ...}
```

### Supported methods

| Method                    | Description                                           |
| ------------------------- | ----------------------------------------------------- |
| `initialize`              | Handshake. Returns protocol version and server info.  |
| `notifications/initialized` | Client notification after handshake. No response.   |
| `tools/list`              | Returns the list of registered tools with schemas.    |
| `tools/call`              | Invoke a tool by name with arguments.                 |

Any other method returns error code `-32601` (method not found).

### Built-in tool: `echo`

The only tool registered in the current `braid mcp` implementation is `echo`. It accepts a
`message` string and returns the same string as its output.

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo","arguments":{"message":"hello"}}}

// Response
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"hello"}]}}
```

### Error codes

| Code     | Meaning                  |
| -------- | ------------------------ |
| `-32700` | Parse error (bad JSON)   |
| `-32601` | Method not found         |
| `-32602` | Tool call failed         |

### Wiring as a tool server

Example Claude Desktop `claude_desktop_config.json` entry:

```json
{
  "mcpServers": {
    "braid": {
      "command": "braid",
      "args": ["mcp"]
    }
  }
}
```

---

## Agent Harness Mode

`braid agent` is the integration point for warpx and editor extensions that consume
structured event streams rather than plain text.

### Session ID via `OZ_RUN_ID`

Set `OZ_RUN_ID` in the environment to inject a session ID from the calling process:

```
OZ_RUN_ID=my-session-42 braid agent --prompt "summarise this file"
```

Without `OZ_RUN_ID` a Unix timestamp is generated.

### Output contract

All output goes to **stdout**. Stderr is reserved for diagnostics only.

1. Zero or more event lines, one JSON object per line, in emission order.
2. Exactly one final `response` line.

The `response` line schema:

```json
{
  "type": "response",
  "text": "<final assistant text>",
  "tokens": {"input": <N>, "output": <M>}
}
```

`tokens` is omitted (`null` in JSON) when the provider does not return token counts.

### Consuming the stream

Read stdout line by line. Lines that do not start with `{` can be ignored. Parse each line
as JSON and dispatch on `kind` (event lines) or `type == "response"` (final line).

---

## Error Model

All commands return exit code `0` on success.

| Exit code | Condition                                                                       |
| --------- | ------------------------------------------------------------------------------- |
| `0`       | Success.                                                                        |
| `1`       | Runtime error: missing prompt, empty stdin, unknown provider name, provider     |
|           | construction failure (e.g. missing `OPENAI_API_KEY` for openai), session store  |
|           | I/O error, engine error, MCP server I/O error.                                  |
| `2`       | Argument parse error (clap): unknown flag, missing required argument, bad       |
|           | subcommand.                                                                     |

Error messages are written to stderr. No structured error format is currently emitted on
stdout.

`braid doctor` always exits `0` even when checks fail — check results are for human review
only.

---

## Environment Variables

| Variable         | Used by              | Effect                                                          |
| ---------------- | -------------------- | --------------------------------------------------------------- |
| `OPENAI_API_KEY` | `run`, `agent`, `doctor` | Selects OpenAI provider in auto-detect; required for openai. |
| `HOME`           | all commands         | Resolves `~/.braid/sessions/`. Required; errors if absent.      |
| `OZ_RUN_ID`      | `agent`              | Session ID injected by warpx. Falls back to Unix timestamp.     |
