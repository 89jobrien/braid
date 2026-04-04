use super::{Check, CheckResult, CheckStatus};

pub struct OpenAiKeyCheck;

impl Check for OpenAiKeyCheck {
    fn run(&self) -> CheckResult {
        if std::env::var("OPENAI_API_KEY").is_ok() {
            CheckResult {
                name: "OPENAI_API_KEY",
                status: CheckStatus::Pass,
                message: "set".into(),
            }
        } else {
            CheckResult {
                name: "OPENAI_API_KEY",
                status: CheckStatus::Fail,
                message: "not set — export OPENAI_API_KEY=sk-...".into(),
            }
        }
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn key_check_pass_when_env_set() {
        unsafe { std::env::set_var("OPENAI_API_KEY", "sk-test") };
        let r = OpenAiKeyCheck.run();
        assert!(matches!(r.status, CheckStatus::Pass));
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
    }

    #[test]
    fn key_check_fail_when_env_missing() {
        let saved = std::env::var("OPENAI_API_KEY").ok();
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
        let r = OpenAiKeyCheck.run();
        assert!(matches!(r.status, CheckStatus::Fail));
        assert!(r.message.contains("not set"));
        if let Some(v) = saved {
            unsafe { std::env::set_var("OPENAI_API_KEY", v) };
        }
    }
}
