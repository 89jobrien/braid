use anyhow::Result;
use braid_core::Provider;
use braid_model::{ContentPart, Message, ProviderRequest, Role};
use braid_providers::MockProvider;

fn verify_provider_contract(provider: &impl Provider) -> Result<()> {
    let request = ProviderRequest {
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "Say hello.".into(),
            }],
        }],
    };

    let response = provider.complete(request)?;

    assert_eq!(response.message.role, Role::Assistant, "response role must be Assistant");
    assert!(!response.message.content.is_empty(), "response content must not be empty");

    let has_text = response.message.content.iter().any(|part| {
        matches!(part, ContentPart::Text { text } if !text.is_empty())
    });
    assert!(has_text, "response must contain at least one non-empty Text part");

    Ok(())
}

#[test]
fn mock_provider_satisfies_contract() {
    let provider = MockProvider;
    verify_provider_contract(&provider).unwrap();
}

#[test]
#[ignore = "requires OPENAI_API_KEY"]
fn openai_provider_satisfies_contract() {
    use braid_providers::OpenAiProvider;

    let provider = OpenAiProvider::default_model()
        .expect("OPENAI_API_KEY must be set");
    let result = verify_provider_contract(&provider);
    assert!(result.is_ok(), "OpenAI contract failed: {}", result.unwrap_err());
}
