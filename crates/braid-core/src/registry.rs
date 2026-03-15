use crate::tools::ToolExecutor;
use anyhow::Result;
use braid_model::{ToolCall, ToolResult};
use std::collections::HashMap;

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, tool: Box<dyn ToolExecutor>) {
        self.tools.insert(name.into(), tool);
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

impl ToolExecutor for ToolRegistry {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        let tool = self
            .tools
            .get(&call.name)
            .ok_or_else(|| anyhow::anyhow!("tool not found: {}", call.name))?;
        tool.execute(call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::StaticTool;

    #[test]
    fn register_and_list_tools() {
        let mut registry = ToolRegistry::new();
        registry.register("beta", Box::new(StaticTool::new("beta", "b output")));
        registry.register("alpha", Box::new(StaticTool::new("alpha", "a output")));
        assert_eq!(registry.list(), vec!["alpha", "beta"]);
    }

    #[test]
    fn execute_dispatches_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register("echo", Box::new(StaticTool::new("echo", "echoed")));
        let result = registry
            .execute(ToolCall {
                id: "call_1".into(),
                name: "echo".into(),
                input: "hello".into(),
            })
            .unwrap();
        assert_eq!(result.name, "echo");
        assert_eq!(result.output, "echoed");
    }

    #[test]
    fn execute_unknown_tool_errors() {
        let registry = ToolRegistry::new();
        let err = registry
            .execute(ToolCall {
                id: "call_1".into(),
                name: "missing".into(),
                input: "".into(),
            })
            .unwrap_err();
        assert!(err.to_string().contains("tool not found: missing"));
    }

    #[test]
    fn get_returns_tool_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register("echo", Box::new(StaticTool::new("echo", "out")));
        assert!(registry.get("echo").is_some());
        assert!(registry.get("missing").is_none());
    }
}
