use braid_model::ToolResult;

use crate::contract::{Hook, HookContext, HookVerdict};

/// A collection of hooks evaluated in registration order.
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
    /// If true, errors during hook evaluation result in Deny.
    pub fail_closed: bool,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: vec![],
            fail_closed: false,
        }
    }

    /// Create a registry that denies on any hook evaluation error.
    pub fn fail_closed() -> Self {
        Self {
            hooks: vec![],
            fail_closed: true,
        }
    }

    /// Register a hook.
    pub fn register(mut self, hook: impl Hook + 'static) -> Self {
        self.hooks.push(Box::new(hook));
        self
    }

    /// Evaluate all hooks. Returns the first Deny verdict, or Allow if all pass.
    /// Hook errors are treated as Deny when `fail_closed` is true, ignored otherwise.
    pub fn evaluate(&self, ctx: &HookContext) -> HookVerdict {
        for hook in &self.hooks {
            match hook.pre_execute(ctx) {
                Ok(HookVerdict::Deny {
                    reason,
                    remediation,
                }) => {
                    return HookVerdict::Deny {
                        reason,
                        remediation,
                    };
                }
                Ok(HookVerdict::Allow) => {}
                Err(e) if self.fail_closed => {
                    return HookVerdict::Deny {
                        reason: format!("hook '{}' failed: {e}", hook.name()),
                        remediation: "check hook configuration".into(),
                    };
                }
                Err(_) => {} // fail open: log and continue
            }
        }
        HookVerdict::Allow
    }

    /// Notify all hooks of a completed tool execution.
    pub fn notify_post(&self, ctx: &HookContext, result: &ToolResult) {
        for hook in &self.hooks {
            hook.post_execute(ctx, result);
        }
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::{SessionId, ToolCall};

    struct AllowHook;
    impl Hook for AllowHook {
        fn name(&self) -> &str {
            "allow"
        }
        fn pre_execute(&self, _ctx: &HookContext) -> anyhow::Result<HookVerdict> {
            Ok(HookVerdict::Allow)
        }
    }

    struct DenyHook(&'static str);
    impl Hook for DenyHook {
        fn name(&self) -> &str {
            self.0
        }
        fn pre_execute(&self, _ctx: &HookContext) -> anyhow::Result<HookVerdict> {
            Ok(HookVerdict::Deny {
                reason: format!("denied by {}", self.0),
                remediation: "none".into(),
            })
        }
    }

    fn make_ctx() -> HookContext {
        HookContext {
            session_id: SessionId("test".into()),
            tool_call: ToolCall {
                id: "call_1".into(),
                name: "test".into(),
                input: "".into(),
            },
        }
    }

    #[test]
    fn empty_registry_allows() {
        let registry = HookRegistry::new();
        assert_eq!(registry.evaluate(&make_ctx()), HookVerdict::Allow);
    }

    #[test]
    fn all_allow_hooks_allows() {
        let registry = HookRegistry::new().register(AllowHook).register(AllowHook);
        assert_eq!(registry.evaluate(&make_ctx()), HookVerdict::Allow);
    }

    #[test]
    fn first_deny_wins() {
        let registry = HookRegistry::new()
            .register(DenyHook("first"))
            .register(DenyHook("second"));
        match registry.evaluate(&make_ctx()) {
            HookVerdict::Deny { reason, .. } => {
                assert!(reason.contains("first"));
            }
            HookVerdict::Allow => panic!("should have been denied"),
        }
    }

    #[test]
    fn deny_after_allow() {
        let registry = HookRegistry::new()
            .register(AllowHook)
            .register(DenyHook("blocker"));
        match registry.evaluate(&make_ctx()) {
            HookVerdict::Deny { reason, .. } => {
                assert!(reason.contains("blocker"));
            }
            HookVerdict::Allow => panic!("should have been denied"),
        }
    }

    struct ErrorHook;
    impl Hook for ErrorHook {
        fn name(&self) -> &str {
            "error-hook"
        }
        fn pre_execute(&self, _ctx: &HookContext) -> anyhow::Result<HookVerdict> {
            Err(anyhow::anyhow!("hook evaluation failed"))
        }
    }

    #[test]
    fn fail_closed_denies_on_hook_error() {
        let registry = HookRegistry::fail_closed().register(ErrorHook);
        match registry.evaluate(&make_ctx()) {
            HookVerdict::Deny { reason, .. } => {
                assert!(reason.contains("error-hook"));
                assert!(reason.contains("hook evaluation failed"));
            }
            HookVerdict::Allow => panic!("fail_closed registry should deny on hook error"),
        }
    }

    #[test]
    fn fail_open_allows_on_hook_error() {
        let registry = HookRegistry::new().register(ErrorHook);
        assert_eq!(registry.evaluate(&make_ctx()), HookVerdict::Allow);
    }
}
