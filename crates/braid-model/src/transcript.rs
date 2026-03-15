use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::session::SessionId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenCount {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub token_count: Option<TokenCount>,
}
