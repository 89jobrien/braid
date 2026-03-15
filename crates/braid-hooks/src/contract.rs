use braid_model::{SessionId, ToolCall, ToolResult};

/// Context passed to hooks for evaluation.
pub struct HookContext {
    pub session_id: SessionId,
    pub tool_call: ToolCall,
}

/// The result of a hook evaluating a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookVerdict {
    /// Allow the tool call to proceed.
    Allow,
    /// Deny the tool call with a reason and remediation hint.
    Deny { reason: String, remediation: String },
}

/// A hook that can inspect and gate tool calls.
pub trait Hook: Send + Sync {
    /// Human-readable name for this hook.
    fn name(&self) -> &str;

    /// Evaluate whether a tool call should proceed.
    fn pre_execute(&self, ctx: &HookContext) -> HookVerdict;

    /// Called after successful tool execution (for logging/auditing).
    fn post_execute(&self, _ctx: &HookContext, _result: &ToolResult) {}
}
