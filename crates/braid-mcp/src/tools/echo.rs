use braid_model::ToolDefinition;
use serde_json::json;

/// Returns the echo tool definition.
pub fn echo_tool() -> ToolDefinition {
    ToolDefinition {
        name: "echo".into(),
        description: "Echoes back the input message".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"]
        }),
    }
}
