pub mod config_dir;
pub mod connectivity;
pub mod keys;
pub mod toolchain;
pub mod tools;
pub mod workspace;

pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

pub trait Check: Send + Sync {
    fn run(&self) -> CheckResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysPass;
    impl Check for AlwaysPass {
        fn run(&self) -> CheckResult {
            CheckResult {
                name: "always_pass",
                status: CheckStatus::Pass,
                message: "fine".into(),
            }
        }
    }

    struct AlwaysFail;
    impl Check for AlwaysFail {
        fn run(&self) -> CheckResult {
            CheckResult {
                name: "always_fail",
                status: CheckStatus::Fail,
                message: "broken".into(),
            }
        }
    }

    #[test]
    fn check_result_carries_name_and_message() {
        let r = AlwaysPass.run();
        assert_eq!(r.name, "always_pass");
        assert!(matches!(r.status, CheckStatus::Pass));
        assert_eq!(r.message, "fine");
    }

    #[test]
    fn fail_result_has_fail_status() {
        let r = AlwaysFail.run();
        assert!(matches!(r.status, CheckStatus::Fail));
    }
}
