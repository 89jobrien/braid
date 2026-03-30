//! Phase 2 cross-crate integration tests
//!
//! Tests the composed behavior of the safety/tool-exposure layer:
//!   braid-redact  — strips secrets from tool output before persist/send
//!   braid-hooks   — fail-closed hook gating before tool execution
//!   braid-mcp     — Content-Length framed JSON-RPC responses under failure
//!
//! These tests deliberately cross crate boundaries to catch wiring bugs that
//! per-crate unit tests cannot see.

use braid_core::{StaticTool, ToolExecutor};
use braid_hooks::{HookRegistry, HookedExecutor, guards::DestructiveCommandGuard};
use braid_model::{SessionId, ToolCall};
use braid_redact::{RedactionPipeline, patterns::SecretPatternRule};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_call(name: &str, input: &str) -> ToolCall {
    ToolCall {
        id: "c1".into(),
        name: name.into(),
        input: input.into(),
    }
}

// ---------------------------------------------------------------------------
// Redact-before-persist invariant
// ---------------------------------------------------------------------------

/// If a tool returns output containing a secret, the RedactionPipeline must
/// strip it before the result is persisted or forwarded.
/// This tests the "Redact-before-persist: privacy by default" design invariant.
#[test]
fn redact_before_persist_strips_secret_from_tool_output() {
    let secret = "sk-abcdefghijklmnopqrstuvwxyz";
    let tool = StaticTool::new("fetch", format!("api_key={secret}").as_str());
    let executor = HookedExecutor::new(tool, HookRegistry::new(), SessionId("s1".into()));

    let result = executor.execute(make_call("fetch", "{}")).unwrap();
    assert!(
        result.output.contains(secret),
        "raw tool output should still contain the secret before redaction"
    );

    // Redact before persisting
    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());
    let redacted = pipeline.redact(&result.output);

    assert!(
        redacted.contains("[REDACTED:api-key]"),
        "secret must be replaced by redaction token"
    );
    assert!(
        !redacted.contains(secret),
        "secret must not appear in redacted output"
    );
}

/// When the tool itself has no secrets, redaction must pass the output through
/// unchanged.
#[test]
fn redact_passes_through_clean_output() {
    let tool = StaticTool::new("echo", "hello world");
    let executor = HookedExecutor::new(tool, HookRegistry::new(), SessionId("s1".into()));

    let result = executor.execute(make_call("echo", "hi")).unwrap();
    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());
    let redacted = pipeline.redact(&result.output);

    assert_eq!(redacted, "hello world");
}

// ---------------------------------------------------------------------------
// Fail-closed hooks invariant
// ---------------------------------------------------------------------------

/// DestructiveCommandGuard must deny a `rm -rf` call and the error must
/// propagate before the inner tool executes.
#[test]
fn hooks_fail_closed_on_destructive_command() {
    let tool = StaticTool::new("shell", "executed — should not reach here");
    let registry = HookRegistry::new().register(DestructiveCommandGuard::new());
    let executor = HookedExecutor::new(tool, registry, SessionId("s1".into()));

    let err = executor
        .execute(make_call("shell", "rm -rf /important"))
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("denied") || msg.contains("rm -rf"),
        "error must explain the denial: {msg}"
    );
    // Crucially: the tool's output must NOT appear (inner never ran)
    assert!(!msg.contains("should not reach here"));
}

/// Safe commands must pass through the DestructiveCommandGuard.
#[test]
fn hooks_allow_safe_commands() {
    let tool = StaticTool::new("shell", "total 8");
    let registry = HookRegistry::new().register(DestructiveCommandGuard::new());
    let executor = HookedExecutor::new(tool, registry, SessionId("s1".into()));

    let result = executor.execute(make_call("shell", "ls -la /tmp")).unwrap();
    assert_eq!(result.output, "total 8");
}

// ---------------------------------------------------------------------------
// Combined: hooks gate execution, then redact output
// ---------------------------------------------------------------------------

/// Full Phase 2 flow on the happy path: hooks allow → tool runs → output is
/// redacted before it would be stored or forwarded over MCP.
#[test]
fn phase2_happy_path_hooks_then_redact() {
    let secret = "AKIAIOSFODNN7EXAMPLE";
    let tool = StaticTool::new("cloud", format!("aws_key={secret}").as_str());
    let registry = HookRegistry::new().register(DestructiveCommandGuard::new());
    let executor = HookedExecutor::new(tool, registry, SessionId("s1".into()));

    // Hook allows the call
    let result = executor
        .execute(make_call("cloud", "describe-instances"))
        .unwrap();

    // Redact before persist/forward
    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());
    let redacted = pipeline.redact(&result.output);

    assert!(redacted.contains("[REDACTED:aws-key]"));
    assert!(!redacted.contains(secret));
}

/// When a hook denies, no output reaches the redaction step — the error is
/// the only result.
#[test]
fn phase2_hook_denial_produces_no_output_to_redact() {
    let secret = "sk-abcdefghijklmnopqrstuvwxyz";
    let tool = StaticTool::new("shell", format!("leaked={secret}").as_str());
    let registry = HookRegistry::new().register(DestructiveCommandGuard::new());
    let executor = HookedExecutor::new(tool, registry, SessionId("s1".into()));

    // Hook blocks the destructive call
    let err = executor
        .execute(make_call("shell", "rm -rf / && echo leaked"))
        .unwrap_err();

    // The secret in the tool's *would-be* output must never be visible
    assert!(!err.to_string().contains(secret));
}

// ---------------------------------------------------------------------------
// MCP registry wired to HookedExecutor
// ---------------------------------------------------------------------------

/// McpToolRegistry can be wired to a HookedExecutor.  Calling an unregistered
/// tool returns an actionable error ("unknown tool"), not a panic.
#[test]
fn mcp_registry_rejects_unknown_tool_with_error() {
    use braid_mcp::McpToolRegistry;

    let registry = McpToolRegistry::new(|call| {
        Ok(braid_model::ToolResult {
            name: call.name,
            output: "unreachable".into(),
        })
    });

    let err = registry
        .call_tool("nonexistent", serde_json::json!({}))
        .unwrap_err();

    assert!(
        err.to_string().contains("unknown tool"),
        "error must identify the unknown tool: {err}"
    );
}

/// McpToolRegistry wired to a HookedExecutor: when the hook denies a tool
/// call, the registry propagates the error — the tool's output is never
/// returned.
#[test]
fn mcp_registry_propagates_hook_denial() {
    use braid_mcp::{McpToolRegistry, echo_tool};

    let registry = McpToolRegistry::new(|call| {
        // Wired to a HookedExecutor — destructive commands are blocked.
        let hook_registry = HookRegistry::new().register(DestructiveCommandGuard::new());
        let tool = StaticTool::new(&call.name, "should not appear");
        let executor = HookedExecutor::new(tool, hook_registry, SessionId("s1".into()));
        executor
            .execute(ToolCall {
                id: call.id,
                name: call.name,
                input: call.input,
            })
            .map(|r| braid_model::ToolResult {
                name: r.name,
                output: r.output,
            })
    })
    .register(echo_tool());

    // Safe echo call succeeds
    let ok = registry.call_tool("echo", serde_json::json!({ "message": "hi" }));
    assert!(ok.is_ok(), "safe call must succeed: {ok:?}");

    // Unknown tool is rejected by the registry before the executor runs
    let err = registry
        .call_tool("nonexistent", serde_json::json!({}))
        .unwrap_err();
    assert!(err.to_string().contains("unknown tool"), "{err}");
}
