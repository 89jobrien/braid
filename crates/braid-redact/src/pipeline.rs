use braid_model::{ContentPart, Event, EventKind, Message};

use crate::rule::RedactionRule;

/// An ordered chain of redaction rules applied sequentially.
pub struct RedactionPipeline {
    rules: Vec<Box<dyn RedactionRule>>,
}

impl RedactionPipeline {
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    /// Add a rule to the pipeline (builder pattern).
    pub fn with_rule(mut self, rule: impl RedactionRule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    /// Apply all rules sequentially to the input string.
    pub fn redact(&self, input: &str) -> String {
        let mut result = input.to_string();
        for rule in &self.rules {
            result = rule.redact(&result);
        }
        result
    }

    /// Redact all text content within a Message, preserving structure.
    pub fn redact_message(&self, msg: &Message) -> Message {
        Message {
            role: msg.role.clone(),
            content: msg
                .content
                .iter()
                .map(|part| self.redact_content_part(part))
                .collect(),
        }
    }

    /// Redact text content within an Event.
    pub fn redact_event(&self, event: &Event) -> Event {
        Event {
            session_id: event.session_id.clone(),
            kind: match &event.kind {
                EventKind::ToolCalled { tool_name } => EventKind::ToolCalled {
                    tool_name: self.redact(tool_name),
                },
                EventKind::ToolCompleted { tool_name } => EventKind::ToolCompleted {
                    tool_name: self.redact(tool_name),
                },
                other => other.clone(),
            },
        }
    }

    fn redact_content_part(&self, part: &ContentPart) -> ContentPart {
        match part {
            ContentPart::Text { text } => ContentPart::Text {
                text: self.redact(text),
            },
            ContentPart::ToolUse { id, name, input } => ContentPart::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: self.redact_json_values(input),
            },
            ContentPart::ToolResult {
                tool_use_id,
                content,
            } => ContentPart::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: self.redact(content),
            },
            ContentPart::Image { .. } => part.clone(),
        }
    }

    fn redact_json_values(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => serde_json::Value::String(self.redact(s)),
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.redact_json_values(v)).collect())
            }
            serde_json::Value::Object(obj) => serde_json::Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), self.redact_json_values(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

impl Default for RedactionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl braid_ports::Redactor for RedactionPipeline {
    fn redact_message(&self, msg: &braid_model::Message) -> braid_model::Message {
        self.redact_message(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::HomePathRule;
    use crate::patterns::{EnvVarRule, SecretPatternRule};
    use braid_model::{Role, SessionId};

    #[test]
    fn empty_pipeline_passes_through() {
        let pipeline = RedactionPipeline::new();
        assert_eq!(pipeline.redact("hello world"), "hello world");
    }

    #[test]
    fn rules_apply_in_order() {
        let pipeline = RedactionPipeline::new()
            .with_rule(SecretPatternRule::new())
            .with_rule(HomePathRule::new());

        let input = "key sk-abcdefghijklmnopqrstuvwxyz at /Users/joe/dev";
        let result = pipeline.redact(input);
        assert!(result.contains("[REDACTED:api-key]"));
        assert!(result.contains("~/dev"));
    }

    #[test]
    fn pipeline_with_all_rules() {
        let pipeline = RedactionPipeline::new()
            .with_rule(SecretPatternRule::new())
            .with_rule(EnvVarRule::new())
            .with_rule(HomePathRule::new());

        let input = "AKIAIOSFODNN7EXAMPLE at /home/user/app with API_KEY=secret123";
        let result = pipeline.redact(input);
        assert!(result.contains("[REDACTED:aws-key]"));
        assert!(result.contains("~/app"));
        assert!(result.contains("API_KEY=[REDACTED]"));
        assert!(!result.contains("secret123"));
    }

    #[test]
    fn redact_message_preserves_structure() {
        let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

        let msg = Message {
            role: Role::User,
            content: vec![
                ContentPart::Text {
                    text: "my key is sk-abcdefghijklmnopqrstuvwxyz".into(),
                },
                ContentPart::Image {
                    media_type: "image/png".into(),
                    data: "base64data".into(),
                },
            ],
        };

        let redacted = pipeline.redact_message(&msg);
        assert_eq!(redacted.role, Role::User);
        assert_eq!(redacted.content.len(), 2);

        match &redacted.content[0] {
            ContentPart::Text { text } => {
                assert!(text.contains("[REDACTED:api-key]"));
                assert!(!text.contains("sk-abcdefghijklmnopqrstuvwxyz"));
            }
            _ => panic!("expected Text"),
        }

        // Image should be unchanged
        match &redacted.content[1] {
            ContentPart::Image { data, .. } => assert_eq!(data, "base64data"),
            _ => panic!("expected Image"),
        }
    }

    #[test]
    fn redact_message_handles_tool_use() {
        let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

        let msg = Message {
            role: Role::Assistant,
            content: vec![ContentPart::ToolUse {
                id: "call_1".into(),
                name: "fetch".into(),
                input: serde_json::json!({
                    "url": "https://api.example.com",
                    "headers": {"Authorization": "Bearer eyJtoken123"}
                }),
            }],
        };

        let redacted = pipeline.redact_message(&msg);
        match &redacted.content[0] {
            ContentPart::ToolUse { input, id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "fetch");
                let auth = input["headers"]["Authorization"].as_str().unwrap();
                assert_eq!(auth, "Bearer [REDACTED]");
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn redact_message_handles_tool_result() {
        let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

        let msg = Message {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult {
                tool_use_id: "call_1".into(),
                content: "result: sk-abcdefghijklmnopqrstuvwxyz".into(),
            }],
        };

        let redacted = pipeline.redact_message(&msg);
        match &redacted.content[0] {
            ContentPart::ToolResult { content, .. } => {
                assert!(content.contains("[REDACTED:api-key]"));
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn redact_event_handles_tool_events() {
        let pipeline = RedactionPipeline::new().with_rule(HomePathRule::new());

        let event = Event {
            session_id: SessionId("s1".into()),
            kind: EventKind::ToolCalled {
                tool_name: "read_file:/Users/joe/secret.txt".into(),
            },
        };

        let redacted = pipeline.redact_event(&event);
        match &redacted.kind {
            EventKind::ToolCalled { tool_name } => {
                assert!(tool_name.contains("~/secret.txt"));
            }
            _ => panic!("expected ToolCalled"),
        }
    }

    #[test]
    fn redact_event_passes_through_simple_events() {
        let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

        let event = Event {
            session_id: SessionId("s1".into()),
            kind: EventKind::SessionStarted,
        };

        let redacted = pipeline.redact_event(&event);
        assert_eq!(redacted.kind, EventKind::SessionStarted);
    }

    #[test]
    fn redaction_pipeline_implements_redactor_port() {
        use crate::patterns::SecretPatternRule;
        use braid_model::{ContentPart, Message, Role};
        use braid_ports::Redactor;

        let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

        // Redactor trait method
        let msg = Message {
            role: Role::User,
            content: vec![ContentPart::Text {
                text: "key: sk-abcdefghijklmnopqrstuvwxyz".into(),
            }],
        };
        let redacted = <RedactionPipeline as Redactor>::redact_message(&pipeline, &msg);
        match &redacted.content[0] {
            ContentPart::Text { text } => {
                assert!(text.contains("[REDACTED:api-key]"));
                assert!(!text.contains("sk-"));
            }
            _ => panic!("expected Text"),
        }
    }
}
