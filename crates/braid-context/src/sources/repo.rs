use anyhow::Result;
use braid_model::ContextChunk;
use chrono::Duration;
use std::path::PathBuf;
use std::process::Command;

use crate::types::ContextSource;

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

fn run_git(root: &PathBuf, args: &[&str]) -> Result<String> {
    let output = Command::new("git").current_dir(root).args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {args:?} failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
