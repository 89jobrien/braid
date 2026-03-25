use anyhow::Result;
use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role};
use braid_ports::Provider;

/// A test double for `Provider` that returns a fixed text response.
pub struct MockProvider {
    response_text: String,
}

impl MockProvider {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            response_text: text.into(),
        }
    }
}

impl Provider for MockProvider {
    fn complete(&self, _request: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentPart::Text {
                    text: self.response_text.clone(),
                }],
            },
            token_count: None,
        })
    }
}
