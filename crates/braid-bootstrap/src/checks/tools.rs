use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

fn version_check(name: &'static str, bin: &str, args: &[&str], warn_only: bool) -> CheckResult {
    match Cmd::new(bin).args(args).output() {
        Ok(out) if out.status.success() => {
            let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
            CheckResult {
                name,
                status: CheckStatus::Pass,
                message: msg,
            }
        }
        _ => CheckResult {
            name,
            status: if warn_only {
                CheckStatus::Warn
            } else {
                CheckStatus::Fail
            },
            message: format!("{bin} not found — install with: cargo install {bin}"),
        },
    }
}

pub struct GitCheck;
impl Check for GitCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("git").arg("--version").output() {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult {
                    name: "git",
                    status: CheckStatus::Pass,
                    message: msg,
                }
            }
            _ => CheckResult {
                name: "git",
                status: CheckStatus::Fail,
                message: "git not found — install via your OS package manager".into(),
            },
        }
    }
}

pub struct DoobCheck;
impl Check for DoobCheck {
    fn run(&self) -> CheckResult {
        version_check("doob", "doob", &["--version"], true)
    }
}

pub struct CargoNextestCheck;
impl Check for CargoNextestCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("cargo")
            .args(["nextest", "--version"])
            .output()
        {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult {
                    name: "cargo-nextest",
                    status: CheckStatus::Pass,
                    message: msg,
                }
            }
            _ => CheckResult {
                name: "cargo-nextest",
                status: CheckStatus::Fail,
                message: "not found — install with: cargo install cargo-nextest".into(),
            },
        }
    }
}

pub struct CargoDenyCheck;
impl Check for CargoDenyCheck {
    fn run(&self) -> CheckResult {
        match std::process::Command::new("cargo")
            .args(["deny", "--version"])
            .output()
        {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                CheckResult {
                    name: "cargo-deny",
                    status: CheckStatus::Pass,
                    message: msg,
                }
            }
            _ => CheckResult {
                name: "cargo-deny",
                status: CheckStatus::Fail,
                message: "not found — install with: cargo install cargo-deny".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_check_runs_without_panic() {
        let r = GitCheck.run();
        assert_eq!(r.name, "git");
        assert!(matches!(r.status, CheckStatus::Pass | CheckStatus::Fail));
    }

    #[test]
    fn doob_check_is_warn_on_missing() {
        let r = DoobCheck.run();
        assert_eq!(r.name, "doob");
        assert!(
            !matches!(r.status, CheckStatus::Fail),
            "doob must warn, not fail"
        );
    }
}
