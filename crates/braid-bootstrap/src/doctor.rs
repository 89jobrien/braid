use crate::checks::{
    CheckResult,
    config_dir::BraidConfigDirCheck,
    connectivity::{OllamaConnectivityCheck, OpenAiConnectivityCheck},
    keys::OpenAiKeyCheck,
    toolchain::RustToolchainCheck,
    tools::{CargoDenyCheck, CargoNextestCheck, DoobCheck, GitCheck},
    workspace::WorkspaceHealthCheck,
};

pub fn run_checks() -> Vec<CheckResult> {
    let checks: Vec<Box<dyn crate::checks::Check>> = vec![
        Box::new(RustToolchainCheck),
        Box::new(GitCheck),
        Box::new(DoobCheck),
        Box::new(CargoNextestCheck),
        Box::new(CargoDenyCheck),
        Box::new(OpenAiKeyCheck),
        Box::new(OllamaConnectivityCheck),
        Box::new(OpenAiConnectivityCheck),
        Box::new(WorkspaceHealthCheck),
        Box::new(BraidConfigDirCheck),
    ];
    checks.into_iter().map(|c| c.run()).collect()
}
