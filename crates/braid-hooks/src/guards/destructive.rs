use crate::contract::{Hook, HookContext, HookVerdict};
use anyhow::Result;

/// Guards against destructive commands by checking tool call input against blocked patterns.
pub struct DestructiveCommandGuard {
    patterns: Vec<(&'static str, &'static str)>,
}

impl DestructiveCommandGuard {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ("rm -rf", "Use targeted rm with specific paths instead"),
                ("DROP TABLE", "Use migrations to manage schema changes"),
                ("DROP DATABASE", "Use migrations to manage schema changes"),
                ("TRUNCATE TABLE", "Use DELETE with WHERE clause instead"),
                (
                    "git push --force",
                    "Use git push --force-with-lease instead",
                ),
                ("git push -f", "Use git push --force-with-lease instead"),
                (
                    "git reset --hard",
                    "Use git stash or create a backup branch first",
                ),
                ("chmod 777", "Use more restrictive permissions (e.g., 755)"),
                (":(){ :|:& };:", "Fork bomb detected — do not execute"),
            ],
        }
    }
}

impl Default for DestructiveCommandGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize input for matching: collapse whitespace runs, lowercase.
fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

impl Hook for DestructiveCommandGuard {
    fn name(&self) -> &str {
        "destructive-command-guard"
    }

    fn pre_execute(&self, ctx: &HookContext) -> Result<HookVerdict> {
        let normalized = normalize(&ctx.tool_call.input);
        for (pattern, remediation) in &self.patterns {
            if normalized.contains(&normalize(pattern)) {
                return Ok(HookVerdict::Deny {
                    reason: format!("blocked destructive pattern: {pattern}"),
                    remediation: remediation.to_string(),
                });
            }
        }
        Ok(HookVerdict::Allow)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use braid_model::{SessionId, ToolCall};

    fn make_ctx(input: &str) -> HookContext {
        HookContext {
            session_id: SessionId("test".into()),
            tool_call: ToolCall {
                id: "call_1".into(),
                name: "shell".into(),
                input: input.into(),
            },
        }
    }

    #[test]
    fn blocks_rm_rf() {
        let guard = DestructiveCommandGuard::new();
        match guard.pre_execute(&make_ctx("rm -rf /")).unwrap() {
            HookVerdict::Deny {
                reason,
                remediation,
            } => {
                assert!(reason.contains("rm -rf"));
                assert!(!remediation.is_empty());
            }
            HookVerdict::Allow => panic!("should have been denied"),
        }
    }

    #[test]
    fn blocks_drop_table() {
        let guard = DestructiveCommandGuard::new();
        assert!(matches!(
            guard.pre_execute(&make_ctx("DROP TABLE users;")).unwrap(),
            HookVerdict::Deny { .. }
        ));
    }

    #[test]
    fn blocks_force_push() {
        let guard = DestructiveCommandGuard::new();
        assert!(matches!(
            guard
                .pre_execute(&make_ctx("git push --force origin main"))
                .unwrap(),
            HookVerdict::Deny { .. }
        ));
        assert!(matches!(
            guard
                .pre_execute(&make_ctx("git push -f origin main"))
                .unwrap(),
            HookVerdict::Deny { .. }
        ));
    }

    #[test]
    fn allows_safe_commands() {
        let guard = DestructiveCommandGuard::new();
        assert_eq!(
            guard.pre_execute(&make_ctx("ls -la")).unwrap(),
            HookVerdict::Allow
        );
        assert_eq!(
            guard.pre_execute(&make_ctx("git status")).unwrap(),
            HookVerdict::Allow
        );
        assert_eq!(
            guard.pre_execute(&make_ctx("cat README.md")).unwrap(),
            HookVerdict::Allow
        );
    }

    #[test]
    fn deny_includes_reason_and_remediation() {
        let guard = DestructiveCommandGuard::new();
        match guard
            .pre_execute(&make_ctx("git reset --hard HEAD~3"))
            .unwrap()
        {
            HookVerdict::Deny {
                reason,
                remediation,
            } => {
                assert!(reason.contains("git reset --hard"));
                assert!(remediation.contains("stash"));
            }
            HookVerdict::Allow => panic!("should have been denied"),
        }
    }

    #[test]
    fn blocks_extra_whitespace_variants() {
        let guard = DestructiveCommandGuard::new();
        assert!(matches!(
            guard.pre_execute(&make_ctx("rm  -rf /")).unwrap(),
            HookVerdict::Deny { .. }
        ));
        assert!(matches!(
            guard
                .pre_execute(&make_ctx("git  push  --force origin main"))
                .unwrap(),
            HookVerdict::Deny { .. }
        ));
    }

    #[test]
    fn blocks_case_variants() {
        let guard = DestructiveCommandGuard::new();
        assert!(matches!(
            guard.pre_execute(&make_ctx("drop table users")).unwrap(),
            HookVerdict::Deny { .. }
        ));
        assert!(matches!(
            guard.pre_execute(&make_ctx("Drop Table users")).unwrap(),
            HookVerdict::Deny { .. }
        ));
        assert!(matches!(
            guard
                .pre_execute(&make_ctx("truncate table events"))
                .unwrap(),
            HookVerdict::Deny { .. }
        ));
    }
}
