use anyhow::Result;
use braid_model::{
    Event, Message, ProviderRequest, ProviderResponse, SessionId, ToolCall, ToolResult,
};
use std::sync::Arc;

// ── Provider ────────────────────────────────────────────────────────────────

pub trait Provider {
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

// ── ToolExecutor ─────────────────────────────────────────────────────────────

pub trait ToolExecutor {
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

// ── Redactor ─────────────────────────────────────────────────────────────────

pub trait Redactor {
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

// ── EventSink ────────────────────────────────────────────────────────────────

pub trait EventSink {
    fn record(&self, event: &Event) -> Result<()>;
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

// ── SessionStorage ───────────────────────────────────────────────────────────

pub trait SessionStorage {
    fn write(&self, id: &SessionId, events: &[Event]) -> Result<()>;
    fn load(&self, id: &SessionId) -> Result<Vec<Event>>;
    fn list(&self) -> Result<Vec<SessionId>>;
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

// ── ContextProvider ──────────────────────────────────────────────────────────

pub trait ContextProvider {
    fn assemble(&self) -> Result<braid_model::ContextSnapshot>;
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

// ── Hook ─────────────────────────────────────────────────────────────────────

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

pub trait Hook: Send + Sync {
    fn name(&self) -> &'static str;
    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict>;
    fn post_execute(&self, _ctx: &HookContext, _result: &ToolResult) {}
}
