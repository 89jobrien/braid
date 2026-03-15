/// A single redaction rule that transforms sensitive content in text.
pub trait RedactionRule: Send + Sync {
    /// Human-readable name for this rule.
    fn name(&self) -> &str;

    /// Apply this rule to the input, returning redacted text.
    fn redact(&self, input: &str) -> String;
}
