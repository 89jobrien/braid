# braid-tui

`braid-tui` is Braid's interactive terminal application. It combines a session inspector with an
agent harness that can send prompts through the same engine, safety, context, provider, and storage
pipeline used by the CLI.

## Workspace role and architecture

This unpublished binary is an alternative composition root built with Ratatui and a Bubble Tea-style
`Model`/`Msg` update loop. Its modules separate pure key/state transitions, session loading, chat
execution, component catalog discovery, completion state, and rendering.

The binary opens `$HOME/.braid/sessions`. It starts in harness mode and requires a terminal of at
least 80 columns by 24 rows.

## Running

```console
cargo run -p braid-tui
```

Provider selection is environment-based:

| Variable | Behavior |
| --- | --- |
| `BRAID_PROVIDER=openai` | Use OpenAI and require `OPENAI_API_KEY` |
| unset or any other value | Use the local Ollama-compatible endpoint |
| `BRAID_MODEL` | Select the model; defaults to `gpt-oss` |

## Modes and controls

### Harness mode

The harness renders component catalog, chat, and context panes. It runs provider work on a
background thread, stores sessions with `tui-<unix-seconds>` IDs, injects Doob and Git context,
applies all built-in redaction rules, and uses a fail-closed destructive-command hook.

| Key | Action |
| --- | --- |
| `Tab` | Cycle catalog, chat, and context focus |
| `Enter` | Send chat input or accept an open completion |
| `/`, `@`, `$` | Open command/prompt, agent, or context completion at a word boundary |
| Up/Down | Move through catalog or completion choices |
| `Esc` | Close completion |
| `F1` | Switch to inspector mode |
| `Ctrl-q` | Quit while chat input is focused |

Slash completion reads commands and prompts from `$HOME/.braid/components`. Agent completion reads
Markdown filenames from `$HOME/.claude/agents`, with built-in fallback names. Dollar completion
offers `session`, `doob`, `repo`, `model`, and `context`. Completion inserts text into the chat
input; the current engine sends the resulting prompt as ordinary text rather than executing
component or agent semantics.

### Inspector mode

| Key | Action |
| --- | --- |
| `Tab` | Switch focus between session list and timeline |
| Up/Down | Move the focused selection |
| `Enter` | Expand/collapse the selected event's JSON payload |
| `r` | Reload sessions from disk |
| `F1` | Switch to harness mode |
| `q` | Quit |

Stored payload text is sanitized before rendering to strip ANSI escape sequences and replace unsafe
control characters.

## Implementation status

Both modes, session replay, component discovery, completion UI, background provider execution,
context injection, redaction, hooks, and terminal restoration on panic are implemented. The context
pane currently shows status and source names rather than a live snapshot. Harness chat state is
in-memory per process and does not reconstruct full conversations from event-only session storage.
The engine tool registry is currently empty in TUI chat, so provider-returned tool calls cannot be
successfully dispatched.

The workspace architecture checker currently flags `braid-tui` because this composition root depends
directly on `braid-providers`, despite the xtask rule classifying it as a support crate.

## Features

This package has no Cargo feature flags and is `publish = false`.

## Development and testing

Tests cover pure navigation, session loading/reload behavior, trigger detection and completion,
catalog behavior, terminal text sanitization, and word wrapping.

```console
cargo nextest run -p braid-tui
cargo clippy -p braid-tui -- -D warnings
cargo check -p braid-tui
```

Interactive behavior must also be checked in a terminal because rendering and focus depend on the
terminal backend and dimensions.
