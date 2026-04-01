pub mod openai;

#[cfg(feature = "test-support")]
pub mod mock;

pub use openai::OpenAiProvider;

#[cfg(feature = "test-support")]
pub use mock::MockProvider;

#[cfg(test)]
mod tests {
    #[cfg(feature = "test-support")]
    #[test]
    fn mock_provider_returns_configured_response() {
        use crate::MockProvider;
        use braid_model::{ContentPart, Message, ProviderRequest, Role};
        use braid_ports::Provider;

        let provider = MockProvider::with_text("hello from mock");
        let req = ProviderRequest {
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: "hi".into() }],
            }],
            tools: vec![],
        };
        let resp = provider.complete(req).unwrap();
        let text = match &resp.message.content[0] {
            ContentPart::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert_eq!(text, "hello from mock");
    }
}
