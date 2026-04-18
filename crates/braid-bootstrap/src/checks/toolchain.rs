use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

pub struct RustToolchainCheck;

impl Check for RustToolchainCheck {
    fn run(&self) -> CheckResult {
        let output = Cmd::new("rustc").arg("--version").output();
        match output {
            Ok(out) if out.status.success() => {
                let raw = String::from_utf8_lossy(&out.stdout);
                let version = raw
                    .trim()
                    .strip_prefix("rustc ")
                    .unwrap_or_else(|| raw.trim());
                let parts: Vec<&str> = version.split('.').collect();
                let ok = parts.len() >= 2
                    && parts[0].parse::<u32>().unwrap_or(0) >= 1
                    && parts[1]
                        .trim_end_matches(|c: char| !c.is_numeric())
                        .parse::<u32>()
                        .unwrap_or(0)
                        >= 88;
                if ok {
                    CheckResult {
                        name: "rust toolchain",
                        status: CheckStatus::Pass,
                        message: format!("rustc {version}"),
                    }
                } else {
                    CheckResult {
                        name: "rust toolchain",
                        status: CheckStatus::Fail,
                        message: format!("found {version}, need >= 1.88"),
                    }
                }
            }
            _ => CheckResult {
                name: "rust toolchain",
                status: CheckStatus::Fail,
                message: "rustc not found".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_toolchain_check_runs_without_panic() {
        let result = RustToolchainCheck.run();
        assert_eq!(result.name, "rust toolchain");
        assert!(matches!(
            result.status,
            CheckStatus::Pass | CheckStatus::Fail
        ));
    }
}
