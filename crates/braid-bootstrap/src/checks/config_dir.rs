use super::{Check, CheckResult, CheckStatus};

pub struct BraidConfigDirCheck;

impl Check for BraidConfigDirCheck {
    fn run(&self) -> CheckResult {
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => {
                return CheckResult {
                    name: "~/.braid dir",
                    status: CheckStatus::Fail,
                    message: "HOME not set".into(),
                };
            }
        };
        let dir = std::path::PathBuf::from(home).join(".braid");
        if dir.exists() {
            CheckResult {
                name: "~/.braid dir",
                status: CheckStatus::Pass,
                message: dir.display().to_string(),
            }
        } else {
            CheckResult {
                name: "~/.braid dir",
                status: CheckStatus::Warn,
                message: "not found — run: braid setup".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_check_returns_warn_when_dir_absent() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", tmp.path()) };
        let r = BraidConfigDirCheck.run();
        assert!(matches!(r.status, CheckStatus::Warn));
        assert!(r.message.contains("braid setup"));
    }
}
