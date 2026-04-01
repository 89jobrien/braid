use super::{Check, CheckResult, CheckStatus};
use std::process::Command as Cmd;

pub struct WorkspaceHealthCheck;

impl Check for WorkspaceHealthCheck {
    fn run(&self) -> CheckResult {
        let output = Cmd::new("cargo").args(["check", "--workspace"]).output();
        match output {
            Ok(out) if out.status.success() => CheckResult {
                name: "workspace health",
                status: CheckStatus::Pass,
                message: "cargo check --workspace ok".into(),
            },
            Ok(_) => CheckResult {
                name: "workspace health",
                status: CheckStatus::Fail,
                message: "cargo check --workspace failed".into(),
            },
            Err(_) => CheckResult {
                name: "workspace health",
                status: CheckStatus::Fail,
                message: "cargo not found".into(),
            },
        }
    }
}
