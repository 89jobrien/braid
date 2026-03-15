pub mod echo;

use anyhow::{Result, bail};
use braid_model::{ToolCall, ToolDefinition, ToolResult};

/// Registry of tools exposed over MCP.
pub struct McpToolRegistry {
    tools: Vec<ToolDefinition>,
    executor: Box<dyn Fn(ToolCall) -> Result<ToolResult> + Send + Sync>,
}

impl McpToolRegistry {
    pub fn new(executor: impl Fn(ToolCall) -> Result<ToolResult> + Send + Sync + 'static) -> Self {
        Self {
            tools: vec![],
            executor: Box::new(executor),
        }
    }

    /// Register a tool definition (builder pattern).
    pub fn register(mut self, def: ToolDefinition) -> Self {
        self.tools.push(def);
        self
    }

    /// List all registered tool definitions.
    pub fn list_tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    /// Call a tool by name with the given input.
    pub fn call_tool(&self, name: &str, input: serde_json::Value) -> Result<ToolResult> {
        if !self.tools.iter().any(|t| t.name == name) {
            bail!("unknown tool: {name}");
        }
        let call = ToolCall {
            id: format!("mcp_{name}"),
            name: name.into(),
            input: input.to_string(),
        };
        (self.executor)(call)
    }
}

#[cfg(test)]
mod tests {
    use super::echo::echo_tool;
    use super::*;

    fn make_registry() -> McpToolRegistry {
        McpToolRegistry::new(|call| {
            let input: serde_json::Value =
                serde_json::from_str(&call.input).unwrap_or(serde_json::Value::Null);
            let message = input["message"]
                .as_str()
                .unwrap_or("no message")
                .to_string();
            Ok(ToolResult {
                name: call.name,
                output: message,
            })
        })
        .register(echo_tool())
    }

    #[test]
    fn list_tools_returns_registered() {
        let registry = make_registry();
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[test]
    fn call_echo_tool() {
        let registry = make_registry();
        let result = registry
            .call_tool("echo", serde_json::json!({"message": "hello"}))
            .unwrap();
        assert_eq!(result.name, "echo");
        assert_eq!(result.output, "hello");
    }

    #[test]
    fn call_unknown_tool_errors() {
        let registry = make_registry();
        let err = registry
            .call_tool("missing", serde_json::json!({}))
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool: missing"));
    }

    #[test]
    fn echo_tool_schema_valid() {
        let def = echo_tool();
        assert_eq!(def.name, "echo");
        let params = &def.parameters;
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["message"].is_object());
        assert!(
            params["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("message"))
        );
    }
}
