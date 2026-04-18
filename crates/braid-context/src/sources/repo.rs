use anyhow::Result;
use braid_model::ContextChunk;
use chrono::Duration;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration as StdDuration;

use crate::types::ContextSource;

const TIMEOUT: StdDuration = StdDuration::from_secs(5);
const MAX_OUTPUT: usize = 1_048_576; // 1 MB

pub struct RepoSource {
    pub root: PathBuf,
}

impl RepoSource {
    pub fn new() -> Self {
        Self {
            root: PathBuf::from("."),
        }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Default for RepoSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextSource for RepoSource {
    fn name(&self) -> &'static str {
        "repo"
    }

    fn staleness_window(&self) -> Duration {
        Duration::minutes(30)
    }

    fn fetch(&self) -> Result<Vec<ContextChunk>> {
        let diff = run_git(&self.root, &["diff", "--stat", "HEAD"])?;
        let log = run_git(&self.root, &["log", "--oneline", "-10"])?;

        let mut content = String::new();
        if !diff.trim().is_empty() {
            content.push_str("### Working tree changes\n");
            content.push_str(&diff);
            content.push('\n');
        }
        if !log.trim().is_empty() {
            content.push_str("### Recent commits\n");
            content.push_str(&log);
        }

        if content.trim().is_empty() {
            return Ok(vec![]);
        }

        Ok(vec![ContextChunk::new("repo", "git status", content)])
    }
}

fn truncate_output(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() > MAX_OUTPUT {
        bytes.truncate(MAX_OUTPUT);
        bytes.extend_from_slice(b"\n[output truncated at 1MB]");
    }
    bytes
}

fn run_with_timeout(mut cmd: Command, timeout: StdDuration) -> Result<std::process::Output> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_) => return Ok(child.wait_with_output()?),
            None if start.elapsed() > timeout => {
                let _ = child.kill();
                anyhow::bail!("subprocess timed out after {timeout:?}");
            }
            None => std::thread::sleep(StdDuration::from_millis(50)),
        }
    }
}

fn run_git(root: &PathBuf, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(args);

    let output = match run_with_timeout(cmd, TIMEOUT) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[braid-context] git {args:?} error: {e}");
            return Ok(String::new());
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {args:?} failed: {stderr}");
    }

    let raw = truncate_output(output.stdout);
    Ok(String::from_utf8_lossy(&raw).into_owned())
}
