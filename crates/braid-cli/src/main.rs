use std::io::{self, IsTerminal, Read};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use braid_core::{Engine, Provider, RunInput, SimpleLoopPlanner, ToolRegistry};
use braid_model::{ContentPart, Message, Role, SessionId};
use braid_providers::{MockProvider, OpenAiProvider};

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
        /// Provider to use (mock or openai; default: auto-detect)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long, default_value = "gpt-4o")]
        model: String,
    },
    /// Check environment health
    Doctor,
}

fn resolve_provider(flag: Option<&str>, model: &str) -> Result<Box<dyn Provider>> {
    let provider_name = match flag {
        Some(name) => name.to_string(),
        None => {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                "openai".into()
            } else {
                "mock".into()
            }
        }
    };

    match provider_name.as_str() {
        "mock" => Ok(Box::new(MockProvider)),
        "openai" => Ok(Box::new(OpenAiProvider::new(model)?)),
        other => bail!("unknown provider: {} (expected 'mock' or 'openai')", other),
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
    stdin.lock().read_to_string(&mut buf)
        .context("failed to read from stdin")?;

    if buf.trim().is_empty() {
        bail!("empty prompt from stdin");
    }

    Ok(buf)
}

fn cmd_run(prompt_arg: Option<String>, provider_flag: Option<String>, model: String) -> Result<()> {
    let provider = resolve_provider(provider_flag.as_deref(), &model)?;
    let prompt = resolve_prompt(prompt_arg)?;

    let engine = Engine::new(ToolRegistry::new(), provider);
    let output = engine.run(RunInput {
        session_id: SessionId("session".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentPart::Text { text: prompt }],
        }],
        max_turns: None,
    }, &SimpleLoopPlanner)?;

    let response_text = match output.provider_response.message.content.first() {
        Some(ContentPart::Text { text }) => text.clone(),
        _ => "non-text response".into(),
    };
    println!("{}", response_text);
    if let Some(tc) = &output.provider_response.token_count {
        eprintln!("tokens: {} in, {} out", tc.input, tc.output);
    }
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    doctor::run_checks()
}

mod doctor {
    use anyhow::Result;
    use std::process::Command as ProcessCommand;

    pub fn run_checks() -> Result<()> {
        check_rust_toolchain();
        check_openai_key();
        check_openai_connectivity();
        check_workspace_health();
        Ok(())
    }

    fn check_rust_toolchain() {
        let output = ProcessCommand::new("rustc").arg("--version").output();
        match output {
            Ok(out) if out.status.success() => {
                let version_str = String::from_utf8_lossy(&out.stdout);
                let version = version_str.trim().strip_prefix("rustc ").unwrap_or(version_str.trim());
                let parts: Vec<&str> = version.split('.').collect();
                if parts.len() >= 2 {
                    let major: u32 = parts[0].parse().unwrap_or(0);
                    let minor: u32 = parts[1].parse().unwrap_or(0);
                    if major >= 1 && minor >= 88 {
                        println!("rust toolchain ... ok ({})", version);
                    } else {
                        println!("rust toolchain ... FAIL (found {}, need >= 1.88)", version);
                    }
                } else {
                    println!("rust toolchain ... FAIL (could not parse version: {})", version);
                }
            }
            _ => println!("rust toolchain ... FAIL (rustc not found)"),
        }
    }

    fn check_openai_key() {
        if std::env::var("OPENAI_API_KEY").is_ok() {
            println!("OPENAI_API_KEY ... set");
        } else {
            println!("OPENAI_API_KEY ... not set");
        }
    }

    fn check_openai_connectivity() {
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("openai connectivity ... skipped (no API key)");
            return;
        }

        use braid_core::engine::Provider;
        use braid_model::{ContentPart, Message, ProviderRequest, Role};
        use braid_providers::OpenAiProvider;

        match OpenAiProvider::new("gpt-4o") {
            Ok(provider) => {
                let request = ProviderRequest {
                    messages: vec![Message {
                        role: Role::User,
                        content: vec![ContentPart::Text {
                            text: "hi".into(),
                        }],
                    }],
                };
                match provider.complete(request) {
                    Ok(_) => println!("openai connectivity ... ok"),
                    Err(e) => println!("openai connectivity ... FAIL ({})", e),
                }
            }
            Err(e) => println!("openai connectivity ... FAIL ({})", e),
        }
    }

    fn check_workspace_health() {
        let output = ProcessCommand::new("cargo")
            .args(["check", "--workspace"])
            .output();
        match output {
            Ok(out) if out.status.success() => println!("workspace health ... ok"),
            Ok(_) => println!("workspace health ... FAIL (cargo check failed)"),
            Err(_) => println!("workspace health ... FAIL (cargo not found)"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { prompt, provider, model } => cmd_run(prompt, provider, model),
        Command::Doctor => cmd_doctor(),
    }
}
