use std::sync::Arc;
use std::sync::mpsc;

use anyhow::Result;
use braid_context::{ContextAssembler, ContextAssemblerProvider, DoobSource, RepoSource};
use braid_engine::{Engine, RunInput, SimpleLoopPlanner, ToolRegistry};
use braid_hooks::{DestructiveCommandGuard, HookRegistry, HookedExecutor};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_observe::SessionStore;
use braid_ports::Provider;
use braid_providers::OpenAiProvider;
use braid_redact::{EnvVarRule, HomePathRule, RedactionPipeline, SecretPatternRule};

use crate::catalog::Catalog;
use crate::completion::{CompletionState, Trigger};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
}

pub enum EngineReply {
    Response(String),
    Error(String),
}

/// Scan input backward from the end for a trigger char at a word boundary.
/// Returns `(trigger, filter)` where filter is the text typed after the trigger.
/// Closes if a space appears between the trigger and the end.
pub fn detect_trigger(input: &str) -> Option<(Trigger, String)> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    for i in (0..n).rev() {
        let c = chars[i];
        if c == ' ' {
            return None; // space before finding a trigger — no active completion
        }
        if let Some(trigger) = Trigger::from_char(c) {
            let at_boundary = i == 0 || chars[i - 1] == ' ';
            if at_boundary {
                let filter: String = chars[i + 1..].iter().collect();
                return Some((trigger, filter));
            }
            return None; // trigger not at word boundary
        }
    }
    None
}

/// Recompute completion state from current input text.
pub fn sync_completion(input: &str, completion: &mut Option<CompletionState>, catalog: &Catalog) {
    match detect_trigger(input) {
        Some((trigger, filter)) => {
            match completion {
                Some(comp) if comp.trigger == trigger => {
                    // Same trigger — just update filter
                    if comp.filter != filter {
                        comp.filter = filter;
                        comp.rebuild(catalog);
                    }
                }
                _ => {
                    // New trigger or switched trigger
                    let mut comp = CompletionState::open(trigger, catalog);
                    comp.filter = filter;
                    comp.rebuild(catalog);
                    *completion = Some(comp);
                }
            }
        }
        None => {
            *completion = None;
        }
    }
}

/// Spawn an engine thread and return the receiver for replies.
pub fn send(
    input_text: &str,
    store: Arc<SessionStore>,
    model: String,
    messages: &mut Vec<ChatMessage>,
) -> Option<mpsc::Receiver<EngineReply>> {
    let text = input_text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    messages.push(ChatMessage {
        role: Role::User,
        text: text.clone(),
    });

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reply = run_engine(text, &store, &model);
        let _ = tx.send(match reply {
            Ok(r) => EngineReply::Response(r),
            Err(e) => EngineReply::Error(e.to_string()),
        });
    });
    Some(rx)
}

fn resolve_provider(model: &str) -> Result<Box<dyn Provider>> {
    if std::env::var("BRAID_PROVIDER").as_deref() == Ok("openai") {
        Ok(Box::new(OpenAiProvider::new(model)?))
    } else {
        Ok(Box::new(OpenAiProvider::ollama(model)))
    }
}

pub fn run_engine(prompt: String, store: &Arc<SessionStore>, model: &str) -> Result<String> {
    let provider = resolve_provider(model)?;

    let redactor = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let session_id = SessionId(format!("tui-{secs}"));

    let ctx_provider = Arc::new(ContextAssemblerProvider::new(ContextAssembler::new(vec![
        Box::new(DoobSource::new()),
        Box::new(RepoSource::new()),
    ])));

    let hooks = HookRegistry::fail_closed().register(DestructiveCommandGuard::new());
    let registry = ToolRegistry::new();
    let tools = HookedExecutor::new(registry, hooks, session_id.clone());

    let engine =
        Engine::new(provider, tools, Arc::clone(store), redactor).with_context(ctx_provider);

    let output = engine.run(
        RunInput {
            session_id,
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: prompt }],
            }],
            max_turns: None,
        },
        &SimpleLoopPlanner,
    )?;

    Ok(match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "(non-text response)".into(),
    })
}
