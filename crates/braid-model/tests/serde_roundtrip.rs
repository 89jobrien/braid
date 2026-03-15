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
fn session_phase_roundtrip() {
    roundtrip(&SessionPhase::Planned);
    roundtrip(&SessionPhase::Running);
    roundtrip(&SessionPhase::WaitingForTool);
    roundtrip(&SessionPhase::Completed);
}

#[test]
fn tool_call_roundtrip() {
    roundtrip(&ToolCall {
        id: "call_1".into(),
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
        session_id: sid.clone(),
        kind: EventKind::ToolCompleted {
            tool_name: "echo".into(),
        },
    });
    roundtrip(&Event { session_id: sid, kind: EventKind::SessionCompleted });
}

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
