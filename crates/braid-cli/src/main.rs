use anyhow::Result;
use braid_core::{Engine, RunInput, StaticTool};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::MockProvider;

fn main() -> Result<()> {
    let engine = Engine::new(StaticTool::new("echo", "tool output"), MockProvider);
    let output = engine.run(RunInput {
        session_id: SessionId("demo-session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "hello from braid".into(),
            }],
        }],
    })?;

    let response_text = match &output.provider_response.message.content[0] {
        ContentPart::Text { text } => text.clone(),
        _ => "non-text response".into(),
    };
    println!("provider: {}", response_text);
    println!("events: {}", output.events.len());
    Ok(())
}
