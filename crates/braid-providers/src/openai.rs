use anyhow::{bail, Context, Result};
use braid_core::Provider;
use braid_model::{
    ContentPart, Message, ProviderRequest, ProviderResponse, Role, TokenCount,
};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OpenAiProvider {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.into(),
            client: reqwest::blocking::Client::new(),
        })
    }

    pub fn default_model() -> Result<Self> {
        Self::new("gpt-4o")
    }

    fn to_openai_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages.iter().map(|msg| self.to_openai_message(msg)).collect()
    }

    fn to_openai_message(&self, msg: &Message) -> Value {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        // Check for tool_call_id (Tool role messages)
        if msg.role == Role::Tool {
            if let Some(ContentPart::ToolResult { tool_use_id, content }) = msg.content.first() {
                return json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                });
            }
        }

        // Check for tool_calls (Assistant messages with ToolUse parts)
        let tool_calls: Vec<Value> = msg.content.iter().filter_map(|part| {
            match part {
                ContentPart::ToolUse { id, name, input } => Some(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": input.to_string(),
                    }
                })),
                _ => None,
            }
        }).collect();

        // Build content array from non-tool parts
        let content_parts: Vec<Value> = msg.content.iter().filter_map(|part| {
            match part {
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
                ContentPart::ToolUse { .. } => None,
                ContentPart::ToolResult { .. } => None,
            }
        }).collect();

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

    fn parse_response(&self, body: Value) -> Result<ProviderResponse> {
        let choices = body["choices"]
            .as_array()
            .context("response missing choices array")?;
        if choices.is_empty() {
            bail!("response has empty choices array");
        }

        let choice_msg = &choices[0]["message"];
        let message = self.parse_message(choice_msg)?;

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

    fn parse_message(&self, msg: &Value) -> Result<Message> {
        let role_str = msg["role"]
            .as_str()
            .context("message missing role")?;
        let role = match role_str {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            other => bail!("unknown role: {}", other),
        };

        let mut content = Vec::new();

        // Parse text content
        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            content.push(ContentPart::Text { text: text.to_string() });
        }

        // Parse tool_calls
        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let func = &tc["function"];
                let name = func["name"].as_str().unwrap_or_default().to_string();
                let arguments_str = func["arguments"].as_str().unwrap_or("{}");
                let input: serde_json::Value = serde_json::from_str(arguments_str)
                    .unwrap_or(json!({}));
                content.push(ContentPart::ToolUse { id, name, input });
            }
        }

        Ok(Message { role, content })
    }
}

impl Provider for OpenAiProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let openai_messages = self.to_openai_messages(&request.messages);

        let body = json!({
            "model": self.model,
            "messages": openai_messages,
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .context("failed to send request to OpenAI")?;

        let status = response.status();
        let response_body: Value = response
            .json()
            .context("failed to parse OpenAI response as JSON")?;

        if !status.is_success() {
            bail!(
                "OpenAI API error ({}): {}",
                status,
                response_body
            );
        }

        self.parse_response(response_body)
    }
}
