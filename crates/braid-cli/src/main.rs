use anyhow::Result;
use braid_core::{Engine, RunInput, StaticTool};
use braid_model::{SessionId, ToolCall};
use braid_providers::MockProvider;

fn main() -> Result<()> {
    let engine = Engine::new(StaticTool::new("echo", "tool output"), MockProvider);
    let output = engine.run(RunInput {
        session_id: SessionId("demo-session".into()),
        prompt: "hello from braid".into(),
        tool: ToolCall {
            name: "echo".into(),
            input: "run".into(),
        },
    })?;

    println!("provider: {}", output.provider_response.message);
    println!("tool: {}", output.tool_result.output);
    println!("events: {}", output.events.len());
    Ok(())
}
