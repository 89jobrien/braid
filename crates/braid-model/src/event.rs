use serde::{Deserialize, Serialize};

use crate::SessionId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub session_id: SessionId,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    SessionStarted,
    ToolCalled {
        tool_name: String,
    },
    ToolCompleted {
        tool_name: String,
    },
    ProviderResponded,
    SessionCompleted,
    /// Explicit marker for events that could not be normalized by an ingester.
    /// This variant is only produced by code that explicitly constructs it;
    /// unrecognized JSONL lines are silently skipped during loading.
    Unknown {
        raw: String,
    },
}
