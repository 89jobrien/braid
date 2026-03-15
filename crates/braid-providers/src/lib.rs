use anyhow::Result;
use braid_core::engine::Provider;
use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role};

#[derive(Debug, Default, Clone)]
pub struct MockProvider;

impl Provider for MockProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        let first_text = request.messages.iter()
            .flat_map(|m| &m.content)
            .find_map(|c| match c {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Ok(ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text {
                    text: format!("mock response to: {}", first_text),
                }],
            },
            token_count: None,
        })
    }
}
