# ADR-001: Event Envelope

**Date:** 2026-04-01
**Status:** Accepted

## Context

Braid needs a single, stable representation for everything that happens during a session — provider calls, tool invocations, redaction, lifecycle transitions. This record must be storable, replayable, and forward-compatible as new event kinds are added.

## Decision

All runtime observations are represented as `Event`, defined in `braid-model`:

```rust
pub struct Event {
    pub session_id: SessionId,
    pub sequence:   u64,           // monotonic, per-session
    pub occurred_at: DateTime<Utc>,
    pub kind:       EventKind,
}

pub enum EventKind {
    SessionStarted { input: RunInput },
    ProviderResponded { response: ProviderResponse },
    ToolCalled { call: ToolCall },
    ToolCompleted { call_id: String, result: ToolResult },
    SessionCompleted { output: RunOutput },
    Unknown { raw: serde_json::Value },  // forward-compat catch-all
}
```

Key invariants:

- **`braid-model` owns the envelope.** No other crate defines parallel event types.
- **`Unknown` is mandatory.** Deserializers that encounter an unrecognized `kind` must produce `Unknown { raw }` rather than erroring. This allows old readers to tolerate new writers.
- **Sequence is assigned by the engine**, not the consumer. Storage and replay layers treat it as read-only.
- **Redaction happens before persistence.** `braid-redact` scrubs `Event` values before they reach `braid-observe`. The envelope shape is preserved; only content is sanitized.

## Consequences

- Adding a new `EventKind` variant is a non-breaking change for stored sessions.
- Replaying a session recorded by a newer Braid version produces `Unknown` events for unrecognized kinds — operators see them rather than losing data.
- `braid-observe` and `braid-tui` must handle `Unknown` without panicking.
