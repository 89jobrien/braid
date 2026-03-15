use anyhow::Result;
use braid_core::engine::Provider;
use braid_core::{Engine, RunInput, StaticTool};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::{MockProvider, OpenAiProvider};

fn main() -> Result<()> {
    let provider: Box<dyn Provider> = if std::env::var("OPENAI_API_KEY").is_ok() {
        Box::new(OpenAiProvider::default_model()?)
    } else {
        Box::new(MockProvider)
    };

    let engine = Engine::new(StaticTool::new("echo", "tool output"), provider);
    let output = engine.run(RunInput {
        session_id: SessionId("demo-session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "Say hello in one sentence.".into(),
            }],
        }],
    })?;

    let response_text = match &output.provider_response.message.content[0] {
        ContentPart::Text { text } => text.clone(),
        _ => "non-text response".into(),
    };
    println!("response: {}", response_text);
    if let Some(tc) = &output.provider_response.token_count {
        println!("tokens: {} in, {} out", tc.input, tc.output);
    }
    println!("events: {}", output.events.len());
    Ok(())
}
