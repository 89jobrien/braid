use std::io::{self, IsTerminal, Read};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use braid_context::{
    ContextAssembler, ContextAssemblerProvider, DoobSource, RefreshContextTool, RepoSource,
};
use braid_core::{Engine, RunInput, SimpleLoopPlanner, ToolRegistry};
use braid_hooks::{DestructiveCommandGuard, HookRegistry, HookedExecutor};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_observe::SessionStore;
use braid_ports::Provider;
use braid_providers::OpenAiProvider;
use braid_redact::{EnvVarRule, HomePathRule, RedactionPipeline, SecretPatternRule};

#[derive(Parser)]
#[command(name = "braid")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a session against a provider
    Run {
        /// Prompt text (reads stdin if omitted)
        prompt: Option<String>,
        /// Provider to use (ollama or openai; default: auto-detect)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long, default_value = "gpt-4o")]
        model: String,
    },
    /// Check environment health
    Doctor,
    /// Set up local braid environment (~/.braid/)
    Setup,
    /// Start MCP server over stdio
    Mcp,
    /// Manage stored sessions
    Sessions {
        #[command(subcommand)]
        action: SessionsCommand,
    },
}

#[derive(Subcommand)]
enum SessionsCommand {
    /// List session IDs, newest first
    List,
    /// Print a session's event timeline
    Show {
        /// Session ID to display
        id: String,
    },
    /// Delete oldest sessions, keeping N most recent
    Prune {
        /// Number of sessions to keep
        #[arg(long, default_value = "50")]
        keep: usize,
    },
}

fn resolve_provider(flag: Option<&str>, model: &str) -> Result<Box<dyn Provider>> {
    let provider_name = match flag {
        Some(name) => name.to_string(),
        None => {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                "openai".into()
            } else {
                "ollama".into()
            }
        }
    };

    match provider_name.as_str() {
        "ollama" => Ok(Box::new(OpenAiProvider::ollama(model))),
        "openai" => Ok(Box::new(OpenAiProvider::new(model)?)),
        other => bail!("unknown provider: {other} (expected 'ollama' or 'openai')"),
    }
}

fn resolve_prompt(arg: Option<String>) -> Result<String> {
    if let Some(prompt) = arg {
        return Ok(prompt);
    }

    let stdin = io::stdin();
    if stdin.is_terminal() {
        bail!("no prompt provided. Usage: braid run \"your prompt\" or pipe via stdin");
    }

    let mut buf = String::new();
    stdin
        .lock()
        .read_to_string(&mut buf)
        .context("failed to read from stdin")?;

    if buf.trim().is_empty() {
        bail!("empty prompt from stdin");
    }

    Ok(buf)
}

fn default_store_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(std::path::PathBuf::from(home)
        .join(".braid")
        .join("sessions"))
}

fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let redactor = RedactionPipeline::new()
        .with_rule(SecretPatternRule::new())
        .with_rule(EnvVarRule::new())
        .with_rule(HomePathRule::new());

    let session_id = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        SessionId(format!("{secs}"))
    };

    // Arc lets cmd_sessions (and any future caller) share the same store instance.
    let store = Arc::new(SessionStore::open(default_store_dir()?)?);

    let summarization_provider: Option<Arc<dyn Provider + Send + Sync>> =
        match OpenAiProvider::new(&model) {
            Ok(p) if std::env::var("OPENAI_API_KEY").is_ok() => Some(Arc::new(p)),
            _ => {
                eprintln!(
                    "note: no provider available for context summarization (OPENAI_API_KEY not set)"
                );
                None
            }
        };

    let mut ctx_assembler = ContextAssembler::new(vec![
        Box::new(DoobSource::new()),
        Box::new(RepoSource::new()),
    ]);
    if let Some(p) = summarization_provider {
        ctx_assembler = ctx_assembler.with_provider(p);
    }
    let ctx_provider = Arc::new(ContextAssemblerProvider::new(ctx_assembler));

    let hooks = HookRegistry::fail_closed().register(DestructiveCommandGuard::new());
    let mut registry = ToolRegistry::new();
    registry.register(
        "refresh_context",
        Box::new(RefreshContextTool {
            provider: Some(ctx_provider.clone()),
        }),
    );
    let tools = HookedExecutor::new(registry, hooks, session_id.clone());

    let engine =
        Engine::new(provider, tools, Arc::clone(&store), redactor).with_context(ctx_provider);
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

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{response_text}");
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    let results = braid_bootstrap::doctor::run_checks();
    braid_bootstrap::render::TerminalRenderer::render(&results);
    Ok(())
}

fn cmd_setup() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let braid_dir = std::path::PathBuf::from(home).join(".braid");
    braid_bootstrap::setup::run(&braid_dir)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            prompt,
            provider,
            model,
        } => cmd_run(prompt, provider, model),
        Command::Doctor => cmd_doctor(),
        Command::Setup => cmd_setup(),
        Command::Mcp => cmd_mcp(),
        Command::Sessions { action } => cmd_sessions(action),
    }
}

fn cmd_sessions(action: SessionsCommand) -> Result<()> {
    use braid_observe::render_session;

    let store_dir = default_store_dir()?;
    let store = SessionStore::open(store_dir)?;

    match action {
        SessionsCommand::List => {
            let ids = store.list()?;
            if ids.is_empty() {
                println!("no sessions found");
            } else {
                for id in ids {
                    println!("{}", id.0);
                }
            }
        }
        SessionsCommand::Show { id } => {
            let sid = SessionId(id);
            let events = store.load(&sid)?;
            let meta = store.load_meta(&sid)?;
            render_session(&events, meta.as_ref(), &mut std::io::stdout())?;
        }
        SessionsCommand::Prune { keep } => {
            let deleted = store.prune(keep)?;
            println!("deleted {deleted} session(s)");
        }
    }
    Ok(())
}

fn cmd_mcp() -> Result<()> {
    use braid_mcp::{McpToolRegistry, echo_tool, run_mcp_server};

    let registry = McpToolRegistry::new(|call| {
        let input: serde_json::Value =
            serde_json::from_str(&call.input).unwrap_or(serde_json::Value::Null);
        let message = input["message"].as_str().unwrap_or(&call.input).to_string();
        Ok(braid_model::ToolResult {
            name: call.name,
            output: message,
        })
    })
    .register(echo_tool());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_mcp_server(registry))
}
