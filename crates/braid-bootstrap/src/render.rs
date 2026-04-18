use crate::checks::{CheckResult, CheckStatus};

pub struct TerminalRenderer;

impl TerminalRenderer {
    pub fn render(results: &[CheckResult]) {
        print!("{}", Self::render_plain(results));
    }

    pub fn render_plain(results: &[CheckResult]) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for r in results {
            let (label, color) = match r.status {
                CheckStatus::Pass => ("ok", "\x1b[32m"),
                CheckStatus::Warn => ("warn", "\x1b[33m"),
                CheckStatus::Fail => ("FAIL", "\x1b[31m"),
            };
            let _ = writeln!(
                out,
                "{:<22} ... {color}{label}\x1b[0m ({})",
                r.name, r.message
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckResult, CheckStatus};

    fn make(name: &'static str, status: CheckStatus, msg: &str) -> CheckResult {
        CheckResult {
            name,
            status,
            message: msg.into(),
        }
    }

    #[test]
    fn render_plain_contains_check_name() {
        let results = vec![make("git", CheckStatus::Pass, "git version 2.44.0")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("git"));
        assert!(out.contains("ok"));
        assert!(out.contains("git version 2.44.0"));
    }

    #[test]
    fn render_plain_warn_contains_warn_label() {
        let results = vec![make("doob", CheckStatus::Warn, "not found")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("warn"));
    }

    #[test]
    fn render_plain_fail_contains_fail_label() {
        let results = vec![make("rust toolchain", CheckStatus::Fail, "not found")];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("FAIL"));
    }

    #[test]
    fn render_plain_multiple_results() {
        let results = vec![
            make("git", CheckStatus::Pass, "ok"),
            make("doob", CheckStatus::Warn, "missing"),
        ];
        let out = TerminalRenderer::render_plain(&results);
        assert!(out.contains("git"));
        assert!(out.contains("doob"));
    }
}
