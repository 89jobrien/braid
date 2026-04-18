use regex::Regex;

use crate::rule::RedactionRule;

/// Redacts known secret patterns: AWS keys, GitHub tokens, Bearer headers, sk- prefixed keys.
pub struct SecretPatternRule {
    patterns: Vec<(Regex, &'static str)>,
}

impl SecretPatternRule {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // AWS access key IDs
                (
                    Regex::new(r"AKIA[0-9A-Z]{16}").expect("valid regex"),
                    "[REDACTED:aws-key]",
                ),
                // GitHub tokens (ghp_, gho_, github_pat_)
                (
                    Regex::new(r"ghp_[A-Za-z0-9]{36,}").expect("valid regex"),
                    "[REDACTED:github-token]",
                ),
                (
                    Regex::new(r"gho_[A-Za-z0-9]{36,}").expect("valid regex"),
                    "[REDACTED:github-token]",
                ),
                (
                    Regex::new(r"github_pat_[A-Za-z0-9_]{22,}").expect("valid regex"),
                    "[REDACTED:github-pat]",
                ),
                // Bearer tokens in headers
                (
                    Regex::new(r"Bearer\s+[A-Za-z0-9\-._~+/]+=*").expect("valid regex"),
                    "Bearer [REDACTED]",
                ),
                // Generic sk- prefixed API keys (OpenAI, Stripe, etc.)
                (
                    Regex::new(r"sk-[A-Za-z0-9]{20,}").expect("valid regex"),
                    "[REDACTED:api-key]",
                ),
            ],
        }
    }
}

impl Default for SecretPatternRule {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionRule for SecretPatternRule {
    fn name(&self) -> &'static str {
        "secret-patterns"
    }

    fn redact(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (pattern, replacement) in &self.patterns {
            result = pattern.replace_all(&result, *replacement).into_owned();
        }
        result
    }
}

/// Redacts values in KEY=value patterns where KEY matches common secret env var names.
pub struct EnvVarRule {
    pattern: Regex,
}

impl EnvVarRule {
    pub fn new() -> Self {
        Self {
            pattern: Regex::new(
                r"(?i)((?:API_KEY|SECRET|PASSWORD|TOKEN|PRIVATE_KEY|ACCESS_KEY|AUTH|CREDENTIAL)(?:_[A-Z_]*)?)\s*=\s*(\S+)",
            )
            .expect("valid regex"),
        }
    }
}

impl Default for EnvVarRule {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionRule for EnvVarRule {
    fn name(&self) -> &'static str {
        "env-vars"
    }

    fn redact(&self, input: &str) -> String {
        self.pattern
            .replace_all(input, "$1=[REDACTED]")
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_aws_keys() {
        let rule = SecretPatternRule::new();
        let input = "key is AKIAIOSFODNN7EXAMPLE ok";
        assert_eq!(rule.redact(input), "key is [REDACTED:aws-key] ok");
    }

    #[test]
    fn redacts_github_tokens() {
        let rule = SecretPatternRule::new();
        let input = "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl";
        assert!(rule.redact(input).contains("[REDACTED:github-token]"));
    }

    #[test]
    fn redacts_github_pat() {
        let rule = SecretPatternRule::new();
        let input = "pat: github_pat_11AAAAAA0aaaaaaaaaaaaaaa_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBbbbbbb";
        assert!(rule.redact(input).contains("[REDACTED:github-pat]"));
    }

    #[test]
    fn redacts_bearer_tokens() {
        let rule = SecretPatternRule::new();
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig";
        assert_eq!(rule.redact(input), "Authorization: Bearer [REDACTED]");
    }

    #[test]
    fn redacts_sk_keys() {
        let rule = SecretPatternRule::new();
        let input = "key: sk-abcdefghijklmnopqrstuvwxyz";
        assert!(rule.redact(input).contains("[REDACTED:api-key]"));
    }

    #[test]
    fn leaves_normal_text_alone() {
        let rule = SecretPatternRule::new();
        let input = "just a normal message with no secrets";
        assert_eq!(rule.redact(input), input);
    }

    #[test]
    fn env_var_redacts_secret_values() {
        let rule = EnvVarRule::new();
        let input = "API_KEY=sk-mykey123 and PASSWORD=hunter2";
        let result = rule.redact(input);
        assert!(result.contains("API_KEY=[REDACTED]"));
        assert!(result.contains("PASSWORD=[REDACTED]"));
    }

    #[test]
    fn env_var_leaves_non_secret_vars() {
        let rule = EnvVarRule::new();
        let input = "PATH=/usr/bin EDITOR=vim";
        assert_eq!(rule.redact(input), input);
    }
}
