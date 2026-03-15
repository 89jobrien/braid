use anyhow::Result;
use braid_core::engine::Provider;
use braid_model::{ProviderRequest, ProviderResponse};

#[derive(Debug, Default, Clone)]
pub struct MockProvider;

impl Provider for MockProvider {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse {
            message: format!("mock provider response to: {}", request.prompt),
        })
    }
}
