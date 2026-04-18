use super::{Check, CheckResult, CheckStatus};

pub struct OllamaConnectivityCheck;

impl Check for OllamaConnectivityCheck {
    fn run(&self) -> CheckResult {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        match client.get("http://localhost:11434/api/tags").send() {
            Ok(resp) if resp.status().is_success() => CheckResult {
                name: "ollama connectivity",
                status: CheckStatus::Pass,
                message: "reachable at http://localhost:11434".into(),
            },
            _ => CheckResult {
                name: "ollama connectivity",
                status: CheckStatus::Warn,
                message: "not reachable — start with: ollama serve".into(),
            },
        }
    }
}

pub struct OpenAiConnectivityCheck;

impl Check for OpenAiConnectivityCheck {
    fn run(&self) -> CheckResult {
        let Ok(key) = std::env::var("OPENAI_API_KEY") else {
            return CheckResult {
                name: "openai connectivity",
                status: CheckStatus::Warn,
                message: "skipped — OPENAI_API_KEY not set".into(),
            };
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1
        });

        match client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&key)
            .json(&body)
            .send()
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 429 => {
                CheckResult {
                    name: "openai connectivity",
                    status: CheckStatus::Pass,
                    message: "reachable".into(),
                }
            }
            Ok(resp) => CheckResult {
                name: "openai connectivity",
                status: CheckStatus::Fail,
                message: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult {
                name: "openai connectivity",
                status: CheckStatus::Fail,
                message: format!("request failed: {e}"),
            },
        }
    }
}
