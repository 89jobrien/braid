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
