use anyhow::Result;
use braid_model::{ToolCall, ToolResult};

pub trait ToolExecutor {
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}

#[derive(Debug, Clone)]
pub struct StaticTool {
    name: String,
    output: String,
}

impl StaticTool {
    pub fn new(name: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            output: output.into(),
        }
    }
}

impl ToolExecutor for StaticTool {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        Ok(ToolResult {
            name: call.name.if_empty_then(self.name.clone()),
            output: self.output.clone(),
        })
    }
}

trait StringFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}
