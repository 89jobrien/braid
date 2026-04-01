use std::sync::Arc;

use anyhow::Result;
use braid_model::{ToolCall, ToolResult};
use braid_ports::{ContextProvider, ToolExecutor};

use crate::provider::ContextAssemblerProvider;

/// A `ToolExecutor` that refreshes and renders the context snapshot.
///
/// Registered under the name `"refresh_context"` in the tool registry.
/// When invoked, it calls `ContextAssemblerProvider::refresh()` and returns
/// the rendered markdown snapshot as the tool output.
pub struct RefreshContextTool {
    pub provider: Option<Arc<ContextAssemblerProvider>>,
}

impl ToolExecutor for RefreshContextTool {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        let output = match &self.provider {
            Some(p) => p.refresh()?.render(),
            None => "No context provider configured.".to_string(),
        };
        Ok(ToolResult {
            name: call.name,
            output,
        })
    }
}
