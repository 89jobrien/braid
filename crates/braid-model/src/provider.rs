use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::transcript::TokenCount;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderResponse {
    pub message: Message,
    pub token_count: Option<TokenCount>,
}
