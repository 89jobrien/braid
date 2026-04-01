use anyhow::Result;
use braid_model::{ContentPart, Message, ProviderRequest, Role, ToolDefinition};
use braid_ports::Provider;
use braid_providers::OpenAiProvider;

fn verify_text_completion(provider: &impl Provider) -> Result<()> {
    let request = ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "Say hello.".into(),
            }],
        }],
        tools: vec![],
    };

    let response = provider.complete(request)?;

    assert_eq!(
        response.message.role,
        Role::Assistant,
        "response role must be Assistant"
    );
    assert!(
        !response.message.content.is_empty(),
        "response content must not be empty"
    );

    let has_text = response
        .message
        .content
        .iter()
        .any(|part| matches!(part, ContentPart::Text { text } if !text.is_empty()));
    assert!(
        has_text,
        "response must contain at least one non-empty Text part"
    );

    Ok(())
}

fn verify_error_on_empty_messages(provider: &impl Provider) -> Result<()> {
    let request = ProviderRequest {
        messages: vec![],
        tools: vec![],
    };

    let result = provider.complete(request);
    assert!(
        result.is_err(),
        "provider must return Err for empty messages"
    );

    Ok(())
}

fn verify_tool_calling(provider: &impl Provider) -> Result<()> {
    let tool = ToolDefinition {
        name: "get_weather".into(),
        description: "Get the current weather for a given city".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["city"]
        }),
    };

    // Step 1: Send a message that should trigger the tool
    let request = ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "What is the current weather in Paris? Use the get_weather tool.".into(),
            }],
        }],
        tools: vec![tool.clone()],
    };

    let response = provider.complete(request)?;

    // Verify response contains a ToolUse part
    let tool_use = response.message.content.iter().find_map(|part| match part {
        ContentPart::ToolUse { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
        _ => None,
    });

    let (tool_call_id, tool_name, _tool_input) =
        tool_use.expect("response must contain a ToolUse content part");

    assert!(!tool_call_id.is_empty(), "tool call id must not be empty");
    assert_eq!(
        tool_name, "get_weather",
        "tool name must match requested tool"
    );

    // Step 2: Send the tool result back and get final response
    let follow_up = ProviderRequest {
        messages: vec![
            Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: "What is the current weather in Paris? Use the get_weather tool.".into(),
                }],
            },
            response.message.clone(),
            Message {
                role: Role::Tool,
                content: vec![ContentPart::ToolResult {
                    tool_use_id: tool_call_id,
                    content: "Sunny, 22°C".into(),
                }],
            },
        ],
        tools: vec![tool],
    };

    let final_response = provider.complete(follow_up)?;

    let has_text = final_response
        .message
        .content
        .iter()
        .any(|part| matches!(part, ContentPart::Text { text } if !text.is_empty()));
    assert!(
        has_text,
        "final response after tool result must contain text"
    );

    Ok(())
}

#[test]
#[ignore = "requires Ollama running locally"]
fn ollama_provider_satisfies_contract() {
    let provider = OpenAiProvider::ollama("qwen2.5:3b");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
    verify_error_on_empty_messages(&provider).unwrap();
}

#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    }
    let provider = OpenAiProvider::default_model().expect("OPENAI_API_KEY must be set");
    verify_text_completion(&provider).unwrap();
    verify_tool_calling(&provider).unwrap();
    verify_error_on_empty_messages(&provider).unwrap();
}

#[test]
fn empty_messages_returns_error() {
    let provider = OpenAiProvider::ollama("any-model");
    let request = ProviderRequest {
        messages: vec![],
        tools: vec![],
    };
    assert!(provider.complete(request).is_err());
}
