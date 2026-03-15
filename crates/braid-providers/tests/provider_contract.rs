use anyhow::Result;
use braid_core::Provider;
use braid_model::{ContentPart, Message, ProviderRequest, Role};
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

#[test]
#[ignore = "requires Ollama running locally"]
fn ollama_provider_satisfies_contract() {
    let provider = OpenAiProvider::ollama("qwen2.5:3b");
    verify_text_completion(&provider).unwrap();
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
}
