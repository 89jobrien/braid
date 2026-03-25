use std::time::Duration;

use crate::contract::{Hook, HookContext, HookVerdict};
use anyhow::Result;

/// Placeholder guard for context freshness checking.
/// Ready for when context carries timestamps.
pub struct FreshnessGuard {
    pub max_age: Duration,
}

impl FreshnessGuard {
    pub fn new(max_age: Duration) -> Self {
        Self { max_age }
    }
}

impl Hook for FreshnessGuard {
    fn name(&self) -> &str {
        "freshness-guard"
    }

    fn pre_execute(&self, _ctx: &HookContext) -> Result<HookVerdict> {
        // Placeholder: always allows. When context carries timestamps,
        // this will check if the context is within max_age.
        Ok(HookVerdict::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{SessionId, ToolCall};

    #[test]
    fn placeholder_always_allows() {
        let guard = FreshnessGuard::new(Duration::from_secs(300));
        let ctx = HookContext {
            session_id: SessionId("test".into()),
            tool_call: ToolCall {
                id: "call_1".into(),
                name: "read".into(),
                input: "file.txt".into(),
            },
        };
        assert_eq!(guard.pre_execute(&ctx).unwrap(), HookVerdict::Allow);
    }
}
