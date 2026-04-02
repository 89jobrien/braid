use anyhow::Result;
use braid_model::{SessionId, ToolCall, ToolResult};
use braid_ports::ToolExecutor;

use crate::contract::{HookContext, HookVerdict};
use crate::registry::HookRegistry;

/// Wraps any `ToolExecutor` with hook-based pre/post execution gating.
pub struct HookedExecutor<T: ToolExecutor> {
    inner: T,
    registry: HookRegistry,
    session_id: SessionId,
}

impl<T: ToolExecutor> HookedExecutor<T> {
    pub const fn new(inner: T, registry: HookRegistry, session_id: SessionId) -> Self {
        Self {
            inner,
            registry,
            session_id,
        }
    }
}

impl<T: ToolExecutor> ToolExecutor for HookedExecutor<T> {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        let ctx = HookContext {
            session_id: self.session_id.clone(),
            tool_call: call.clone(),
        };

        match self.registry.evaluate(&ctx) {
            HookVerdict::Allow => {
                let result = self.inner.execute(call)?;
                self.registry.notify_post(&ctx, &result);
                Ok(result)
            }
            HookVerdict::Deny {
                reason,
                remediation,
            } => Err(anyhow::anyhow!(
                "hook denied tool call '{}': {} (remediation: {})",
                call.name,
                reason,
                remediation
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Hook, HookContext, HookVerdict};

    struct FixedTool(&'static str);
    impl ToolExecutor for FixedTool {
        fn execute(&self, call: braid_model::ToolCall) -> anyhow::Result<braid_model::ToolResult> {
            Ok(braid_model::ToolResult {
                name: call.name,
                output: self.0.into(),
            })
        }
    }

    struct BlockAllHook;
    impl Hook for BlockAllHook {
        fn name(&self) -> &'static str {
            "block-all"
        }
        fn pre_execute(&self, _ctx: &HookContext) -> anyhow::Result<HookVerdict> {
            Ok(HookVerdict::Deny {
                reason: "blocked".into(),
                remediation: "don't".into(),
            })
        }
    }

    #[test]
    fn hooked_executor_allows_when_no_hooks() {
        let inner = FixedTool("hello");
        let executor = HookedExecutor::new(inner, HookRegistry::new(), SessionId("test".into()));
        let result = executor
            .execute(ToolCall {
                id: "c1".into(),
                name: "echo".into(),
                input: "hi".into(),
            })
            .expect("should succeed");
        assert_eq!(result.output, "hello");
    }

    #[test]
    fn hooked_executor_denies_blocked_call() {
        let inner = FixedTool("hello");
        let registry = HookRegistry::new().register(BlockAllHook);
        let executor = HookedExecutor::new(inner, registry, SessionId("test".into()));
        let err = executor
            .execute(ToolCall {
                id: "c1".into(),
                name: "echo".into(),
                input: "hi".into(),
            })
            .expect_err("should fail");
        assert!(err.to_string().contains("blocked"));
        assert!(err.to_string().contains("remediation"));
    }

    #[test]
    fn hooked_executor_with_destructive_guard() {
        use crate::guards::DestructiveCommandGuard;

        let inner = FixedTool("output");
        let registry = HookRegistry::new().register(DestructiveCommandGuard::new());
        let executor = HookedExecutor::new(inner, registry, SessionId("test".into()));

        // Safe command passes
        let result = executor
            .execute(ToolCall {
                id: "c1".into(),
                name: "shell".into(),
                input: "ls -la".into(),
            })
            .expect("should succeed");
        assert_eq!(result.output, "output");

        // Destructive command blocked
        let err = executor
            .execute(ToolCall {
                id: "c2".into(),
                name: "shell".into(),
                input: "rm -rf /".into(),
            })
            .expect_err("should fail");
        assert!(err.to_string().contains("rm -rf"));
    }
}
