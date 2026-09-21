//! Hexagonal port traits connecting the Braid runtime to external adapters.

#![cfg_attr(test, allow(clippy::unwrap_used))]
use anyhow::Result;
use braid_model::{
    Event, Message, ProviderRequest, ProviderResponse, SessionId, ToolCall, ToolResult,
};
use std::path::Path;
use std::sync::Arc;

/// Completes provider-neutral model requests.
pub trait Provider {
    /// Sends a request and returns the provider's normalized response.
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse>;
}

impl<T: Provider + ?Sized> Provider for Box<T> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}

impl<T: Provider + ?Sized> Provider for Arc<T> {
    fn complete(&self, request: ProviderRequest) -> Result<ProviderResponse> {
        (**self).complete(request)
    }
}

/// Executes normalized tool calls.
pub trait ToolExecutor {
    /// Runs one tool call and returns its normalized result.
    fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}

impl<T: ToolExecutor + ?Sized> ToolExecutor for Box<T> {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        (**self).execute(call)
    }
}

impl<T: ToolExecutor + ?Sized> ToolExecutor for Arc<T> {
    fn execute(&self, call: ToolCall) -> Result<ToolResult> {
        (**self).execute(call)
    }
}

/// Removes sensitive content before messages cross trust boundaries.
pub trait Redactor {
    /// Returns a structurally equivalent message with sensitive values redacted.
    fn redact_message(&self, msg: &Message) -> Message;
}

impl<T: Redactor + ?Sized> Redactor for Arc<T> {
    fn redact_message(&self, msg: &Message) -> Message {
        (**self).redact_message(msg)
    }
}

impl<T: Redactor + ?Sized> Redactor for Box<T> {
    fn redact_message(&self, msg: &Message) -> Message {
        (**self).redact_message(msg)
    }
}

/// Records runtime events and flushes buffered observations.
pub trait EventSink {
    /// Records one session event.
    fn record(&self, event: &Event) -> Result<()>;
    /// Persists any buffered events.
    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

impl<T: EventSink + ?Sized> EventSink for Arc<T> {
    fn record(&self, event: &Event) -> Result<()> {
        (**self).record(event)
    }
    fn flush(&self) -> Result<()> {
        (**self).flush()
    }
}

impl<T: EventSink + ?Sized> EventSink for Box<T> {
    fn record(&self, event: &Event) -> Result<()> {
        (**self).record(event)
    }
    fn flush(&self) -> Result<()> {
        (**self).flush()
    }
}

/// Persists and manages complete session event streams.
pub trait SessionStorage {
    /// Replaces the stored event stream for a session.
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()>;
    /// Loads a session's events in persisted order.
    fn load(&self, id: &SessionId) -> Result<Vec<Event>>;
    /// Lists available session identifiers.
    fn list(&self) -> Result<Vec<SessionId>>;
    /// Removes older sessions while retaining `keep` entries.
    fn prune(&self, keep: usize) -> Result<usize>;
}

impl<T: SessionStorage + ?Sized> SessionStorage for Arc<T> {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()> {
        (**self).write(id, events)
    }
    fn load(&self, id: &SessionId) -> Result<Vec<Event>> {
        (**self).load(id)
    }
    fn list(&self) -> Result<Vec<SessionId>> {
        (**self).list()
    }
    fn prune(&self, keep: usize) -> Result<usize> {
        (**self).prune(keep)
    }
}

/// Supplies assembled context snapshots to the runtime.
pub trait ContextProvider {
    /// Builds a context snapshot from current source data.
    fn assemble(&self) -> Result<braid_model::ContextSnapshot>;
    /// Rebuilds context using any provider-maintained prior state.
    fn refresh(&self) -> Result<braid_model::ContextSnapshot>;
}

impl<T: ContextProvider + ?Sized> ContextProvider for Box<T> {
    fn assemble(&self) -> Result<braid_model::ContextSnapshot> {
        (**self).assemble()
    }
    fn refresh(&self) -> Result<braid_model::ContextSnapshot> {
        (**self).refresh()
    }
}

impl<T: ContextProvider + ?Sized> ContextProvider for Arc<T> {
    fn assemble(&self) -> Result<braid_model::ContextSnapshot> {
        (**self).assemble()
    }
    fn refresh(&self) -> Result<braid_model::ContextSnapshot> {
        (**self).refresh()
    }
}

#[derive(Debug, Clone)]
pub struct HookContext {
    pub session_id: SessionId,
    pub tool_call: ToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookVerdict {
    Allow,
    Deny { reason: String, remediation: String },
}

/// Applies policy before and after tool execution.
pub trait Hook: Send + Sync {
    /// Returns the stable policy identifier used in diagnostics.
    fn name(&self) -> &'static str;
    /// Allows or denies a tool call before execution.
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    /// Observes a completed tool call and its result.
    fn post_execute(&self, _ctx: &HookContext, _result: &ToolResult) {}
}

/// Metadata for a registered component. Returned by registry queries.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

pub trait ComponentRegistry {
    /// Load a single component from a directory path and register it.
    fn load(&mut self, path: &Path) -> Result<()>;
    /// Return metadata for a component by name.
    fn get(&self, name: &str) -> Option<ComponentInfo>;
    /// List metadata for all registered components, sorted by name.
    fn list(&self) -> Vec<ComponentInfo>;
}
