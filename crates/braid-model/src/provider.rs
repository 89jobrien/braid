use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::tool::ToolDefinition;
use crate::transcript::TokenCount;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequest {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderResponse {
    pub message: Message,
    pub token_count: Option<TokenCount>,
}
