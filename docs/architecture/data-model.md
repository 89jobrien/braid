# Data Model

All types live in `braid-model`. Every other crate imports from here — no parallel domain models.

## Type Hierarchy

```
Session
└── SessionId(String)                  ← opaque identifier
└── SessionPhase                       ← Planned | Running | WaitingForTool | Completed

Conversation
└── Message
    ├── role: Role                     ← System | User | Assistant | Tool
    └── content: Vec<ContentPart>
        ├── Text { text }
        ├── Image { media_type, data } ← base64
        ├── ToolUse { id, name, input }← LLM's tool call request
        └── ToolResult { tool_use_id, content }  ← tool execution result

Tool
├── ToolDefinition { name, description, parameters: Value }  ← schema sent to LLM
├── ToolCall       { id, name, input: String }               ← parsed from ToolUse
└── ToolResult     { name, output: String }

Provider
├── ProviderRequest  { messages: Vec<Message>, tools: Vec<ToolDefinition> }
└── ProviderResponse { message: Message, token_count: Option<TokenCount> }

Transcript
└── Transcript { session_id, messages: Vec<Message>, token_count: Option<TokenCount> }

Token
└── TokenCount { input: u64, output: u64 }

Task
└── TaskContext { task_id: Option<String>, summary: String }

Event                                  ← persisted as JSONL
└── Event { session_id: SessionId, kind: EventKind }
    └── EventKind
        ├── SessionStarted
        ├── ProviderResponded
        ├── ToolCalled    { tool_name: String }
        ├── ToolCompleted { tool_name: String }
        ├── SessionCompleted
        └── Unknown { raw: String }    ← forward-compat escape hatch
```

## Event JSONL Format

Events are persisted as newline-delimited JSON. The format is golden-tested and stable.

```jsonl
{"session_id":"1743300000","kind":"SessionStarted"}
{"session_id":"1743300000","kind":"ProviderResponded"}
{"session_id":"1743300000","kind":{"ToolCalled":{"tool_name":"bash"}}}
{"session_id":"1743300000","kind":{"ToolCompleted":{"tool_name":"bash"}}}
{"session_id":"1743300000","kind":"SessionCompleted"}
```

Unit variants serialize as plain strings. Struct variants serialize as `{"VariantName": {...}}`.

The `Unknown { raw }` variant is never produced by deserialization — unrecognized lines are silently skipped. It exists for explicit use by migration tooling and ingesters that want to preserve unrecognized events.

## Message Flow Through the Engine

```
User prompt
    │
    ▼  Role::User, ContentPart::Text
Engine receives Vec<Message>
    │
    ▼  Engine::with_redactor applied here
ProviderRequest { messages, tools }
    │
    ▼  Provider::complete()
ProviderResponse { message: Message, token_count }
    │         message may contain:
    │           ContentPart::Text       → plain response
    │           ContentPart::ToolUse    → LLM wants to call a tool
    │
    ▼  SimpleLoopPlanner extracts ToolUse parts
ToolCall { id, name, input: String }
    │
    ▼  ToolExecutor::execute()
ToolResult { name, output: String }
    │
    ▼  appended to messages as:
ContentPart::ToolResult { tool_use_id, content }
    │
    ▼  loop continues until Finish action
```

## Session Meta

Written atomically to `meta.json` alongside the session's `events.jsonl`:

```json
{
  "session_id": "1743300000",
  "written_at": "2026-03-30T03:00:00Z",
  "event_count": 5
}
```
