use anyhow::{Context, Result, bail};
use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role, TokenCount};
use braid_ports::Provider;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::blocking::Client,
}

fn build_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

impl OpenAiProvider {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.into(),
            base_url: "https://api.openai.com/v1".into(),
            client: build_client(),
        })
    }

    pub fn default_model() -> Result<Self> {
        Self::new("gpt-4o")
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
            client: build_client(),
        }
    }

    pub fn ollama(model: impl Into<String>) -> Self {
        Self::with_base_url("http://localhost:11434/v1", model, "ollama")
    }

    fn to_openai_messages(messages: &[Message]) -> Vec<Value> {
        messages.iter().map(Self::to_openai_message).collect()
    }

    fn to_openai_message(msg: &Message) -> Value {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        // Check for tool_call_id (Tool role messages)
        if msg.role == Role::Tool
            && let Some(ContentPart::ToolResult {
                tool_use_id,
                content,
            }) = msg.content.first()
        {
            return json!({
                "role": "tool",
                "tool_call_id": tool_use_id,
                "content": content,
            });
        }

        // Check for tool_calls (Assistant messages with ToolUse parts)
        let tool_calls: Vec<Value> = msg
            .content
            .iter()
            .filter_map(|part| match part {
                ContentPart::ToolUse { id, name, input } => Some(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": input.to_string(),
                    }
                })),
                _ => None,
            })
            .collect();

        // Build content array from non-tool parts
        let content_parts: Vec<Value> = msg
            .content
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(json!({
                    "type": "text",
                    "text": text,
                })),
                ContentPart::Image { media_type, data } => Some(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", media_type, data),
                    }
                })),
                ContentPart::ToolUse { .. } | ContentPart::ToolResult { .. } => None,
            })
            .collect();

        let mut msg_json = json!({ "role": role });

        if !tool_calls.is_empty() {
            msg_json["tool_calls"] = json!(tool_calls);
        }

        // Use string content if it's a single text part, array otherwise
        if content_parts.len() == 1 && content_parts[0].get("type") == Some(&json!("text")) {
            msg_json["content"] = content_parts[0]["text"].clone();
        } else if !content_parts.is_empty() {
            msg_json["content"] = json!(content_parts);
        }

        msg_json
    }

    fn build_request_body(
        &self,
        messages: &[Message],
        tools: &[braid_model::ToolDefinition],
    ) -> Value {
        let openai_messages = Self::to_openai_messages(messages);
        let mut body = json!({
            "model": self.model,
            "messages": openai_messages,
        });

        if !tools.is_empty() {
            let tools_json: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
        }

        body
    }

    fn parse_response(body: &Value) -> Result<ProviderResponse> {
        let choices = body["choices"]
            .as_array()
            .context("response missing choices array")?;
        if choices.is_empty() {
            bail!("response has empty choices array");
        }

        let choice_msg = &choices[0]["message"];
        let message = Self::parse_message(choice_msg)?;

        let token_count = body.get("usage").and_then(|usage| {
            let input = usage.get("prompt_tokens")?.as_u64()?;
            let output = usage.get("completion_tokens")?.as_u64()?;
            Some(TokenCount { input, output })
        });

        Ok(ProviderResponse {
            message,
            token_count,
        })
    }

    fn parse_message(msg: &Value) -> Result<Message> {
        let role_str = msg["role"].as_str().context("message missing role")?;
        let role = match role_str {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            other => bail!("unknown role: {other}"),
        };

        let mut content = Vec::new();

        // Parse text content
        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            content.push(ContentPart::Text {
                text: text.to_string(),
            });
        }

        // Parse tool_calls
        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let func = &tc["function"];
                let name = func["name"].as_str().unwrap_or_default().to_string();
                let arguments_str = func["arguments"].as_str().unwrap_or("{}");
                let input: serde_json::Value =
                    serde_json::from_str(arguments_str).unwrap_or_else(|err| {
                        tracing::warn!(
                            error = %err,
                            raw = arguments_str,
                            tool_name = %name,
                            "malformed tool-call JSON arguments; falling back to empty object"
                        );
                        json!({})
                    });
                content.push(ContentPart::ToolUse { id, name, input });
            }
        }

        Ok(Message { role, content })
    }
}

impl Provider for OpenAiProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        if request.messages.is_empty() {
            bail!("cannot complete with empty messages");
        }

        let body = self.build_request_body(&request.messages, &request.tools);

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .context("failed to send request")?;

        let status = response.status();
        let response_body: Value = response
            .json()
            .context("failed to parse OpenAI response as JSON")?;

        if !status.is_success() {
            bail!("OpenAI API error ({status}): {response_body}");
        }

        Self::parse_response(&response_body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_constructor_sets_base_url() {
        let provider = OpenAiProvider::ollama("qwen2.5:3b");
        assert_eq!(provider.base_url, "http://localhost:11434/v1");
        assert_eq!(provider.model, "qwen2.5:3b");
    }

    #[test]
    fn with_base_url_constructor() {
        let provider = OpenAiProvider::with_base_url("http://custom:8080/v1", "my-model", "my-key");
        assert_eq!(provider.base_url, "http://custom:8080/v1");
        assert_eq!(provider.model, "my-model");
        assert_eq!(provider.api_key, "my-key");
    }

    #[test]
    fn rejects_empty_messages() {
        let provider = OpenAiProvider::ollama("qwen2.5:3b");
        let request = ProviderRequest {
            messages: vec![],
            tools: vec![],
        };
        let err = provider.complete(request).expect_err("should fail");
        assert!(
            err.to_string().contains("empty"),
            "expected empty messages error, got: {err}"
        );
    }

    #[test]
    fn serializes_tool_definitions_in_request_body() {
        use braid_model::ToolDefinition;

        let provider = OpenAiProvider::ollama("test-model");
        let tools = vec![ToolDefinition {
            name: "get_weather".into(),
            description: "Get weather for a city".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"]
            }),
        }];

        let body = provider.build_request_body(
            &[Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: "test".into(),
                }],
            }],
            &tools,
        );

        let tools_json = body["tools"].as_array().expect("tools should be an array");
        assert_eq!(tools_json.len(), 1);
        assert_eq!(tools_json[0]["type"], "function");
        assert_eq!(tools_json[0]["function"]["name"], "get_weather");
    }

    #[test]
    fn omits_tools_key_when_empty() {
        let provider = OpenAiProvider::ollama("test-model");
        let body = provider.build_request_body(
            &[Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: "test".into(),
                }],
            }],
            &[],
        );

        assert!(
            body.get("tools").is_none(),
            "tools key should be absent when empty"
        );
    }

    // -------------------------------------------------------------------------
    // Request serialization contracts (Ollama-compatible wire format)
    // These tests verify the exact JSON the provider sends on the wire.
    // They run without a live server and must pass in CI.
    // -------------------------------------------------------------------------

    #[test]
    fn serializes_text_message_as_string_content() {
        let msg = Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "hello".into(),
            }],
        };
        let j = OpenAiProvider::to_openai_message(&msg);
        assert_eq!(j["role"], "user");
        assert_eq!(
            j["content"], "hello",
            "single text part must serialize as a plain string"
        );
    }

    #[test]
    fn serializes_assistant_tool_use_message() {
        let msg = Message {
            role: Role::Assistant,
            content: vec![ContentPart::ToolUse {
                id: "tc-1".into(),
                name: "echo".into(),
                input: serde_json::json!({ "msg": "hi" }),
            }],
        };
        let j = OpenAiProvider::to_openai_message(&msg);
        assert_eq!(j["role"], "assistant");
        let calls = j["tool_calls"]
            .as_array()
            .expect("tool_calls should be present");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "tc-1");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "echo");
        let args: Value = serde_json::from_str(
            calls[0]["function"]["arguments"]
                .as_str()
                .expect("should succeed"),
        )
        .expect("should succeed");
        assert_eq!(args["msg"], "hi");
    }

    #[test]
    fn serializes_tool_result_message() {
        let msg = Message {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult {
                tool_use_id: "tc-1".into(),
                content: "42".into(),
            }],
        };
        let j = OpenAiProvider::to_openai_message(&msg);
        assert_eq!(j["role"], "tool");
        assert_eq!(j["tool_call_id"], "tc-1");
        assert_eq!(j["content"], "42");
    }

    // -------------------------------------------------------------------------
    // Response parsing contracts
    // -------------------------------------------------------------------------

    fn mock_text_response(text: &str) -> Value {
        serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": text } }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        })
    }

    fn mock_tool_call_response(tool_name: &str, args_json: &str) -> Value {
        serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "tc-1",
                        "type": "function",
                        "function": { "name": tool_name, "arguments": args_json }
                    }]
                }
            }]
        })
    }

    #[test]
    fn parses_text_response_with_token_count() {
        let resp =
            OpenAiProvider::parse_response(&mock_text_response("hello")).expect("should succeed");
        assert_eq!(resp.message.role, Role::Assistant);
        assert_eq!(
            resp.message.content,
            vec![ContentPart::Text {
                text: "hello".into()
            }]
        );
        let tc = resp.token_count.expect("token_count should be present");
        assert_eq!(tc.input, 10);
        assert_eq!(tc.output, 5);
    }

    #[test]
    fn parses_response_without_usage_field() {
        let body = serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "hi" } }]
        });
        let resp = OpenAiProvider::parse_response(&body).expect("should succeed");
        assert!(
            resp.token_count.is_none(),
            "token_count should be None when usage absent"
        );
    }

    #[test]
    fn parses_tool_call_response() {
        let resp =
            OpenAiProvider::parse_response(&mock_tool_call_response("echo", r#"{"msg":"hi"}"#))
                .expect("should succeed");
        assert_eq!(resp.message.role, Role::Assistant);
        match &resp.message.content[0] {
            ContentPart::ToolUse { id, name, input } => {
                assert_eq!(id, "tc-1");
                assert_eq!(name, "echo");
                assert_eq!(input["msg"], "hi");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn malformed_tool_call_arguments_falls_back_to_empty_object() {
        // Invalid JSON in arguments — must not panic, must fall back to {}
        let resp = OpenAiProvider::parse_response(&mock_tool_call_response("echo", "NOT JSON"))
            .expect("should succeed despite malformed arguments");
        match &resp.message.content[0] {
            ContentPart::ToolUse { name, input, .. } => {
                assert_eq!(name, "echo");
                assert_eq!(
                    *input,
                    serde_json::json!({}),
                    "malformed args must become {{}}"
                );
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_errors_on_missing_choices() {
        let err = OpenAiProvider::parse_response(&serde_json::json!({})).expect_err("should fail");
        assert!(err.to_string().contains("choices"));
    }

    #[test]
    fn parse_response_errors_on_empty_choices() {
        let err = OpenAiProvider::parse_response(&serde_json::json!({ "choices": [] }))
            .expect_err("should fail");
        assert!(err.to_string().contains("empty"));
    }
}
