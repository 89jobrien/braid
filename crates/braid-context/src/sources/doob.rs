use anyhow::Result;
use braid_model::ContextChunk;
use chrono::Duration;
use std::process::Command;
use std::time::Duration as StdDuration;

use crate::types::ContextSource;

const TIMEOUT: StdDuration = StdDuration::from_secs(5);
const MAX_OUTPUT: usize = 1_048_576; // 1 MB

pub struct DoobSource {
    pub project: Option<String>,
}

impl DoobSource {
    pub fn new() -> Self {
        Self { project: None }
    }

    pub fn with_project(project: impl Into<String>) -> Self {
        Self {
            project: Some(project.into()),
        }
    }
}

impl Default for DoobSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextSource for DoobSource {
    fn name(&self) -> &'static str {
        "doob"
    }

    fn staleness_window(&self) -> Duration {
        Duration::hours(1)
    }

    fn fetch(&self) -> Result<Vec<ContextChunk>> {
        let mut cmd = Command::new("doob");
        cmd.args(["todo", "list", "--format", "json"]);
        if let Some(proj) = &self.project {
            cmd.args(["--project", proj]);
        }

        let output = match run_with_timeout(cmd, TIMEOUT) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[braid-context] doob subprocess error: {e}");
                return Ok(vec![]);
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[braid-context] doob exited with error: {stderr}");
            return Ok(vec![]);
        }

        let raw = truncate_output(output.stdout);
        let stdout = String::from_utf8_lossy(&raw);

        let todos: Vec<serde_json::Value> = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[braid-context] doob JSON parse error: {e}");
                return Ok(vec![]);
            }
        };

        if todos.is_empty() {
            return Ok(vec![]);
        }

        let content = todos
            .iter()
            .filter_map(|t| {
                let text = t.get("text").or_else(|| t.get("title"))?.as_str()?;
                let status = t
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                Some(format!("[{status}] {text}"))
            })
            .collect::<Vec<_>>()
            .join("\n");

        if content.is_empty() {
            return Ok(vec![]);
        }

        Ok(vec![ContextChunk::new("doob", "current todos", content)])
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
                anyhow::bail!("subprocess timed out after {:?}", timeout);
            }
            None => std::thread::sleep(StdDuration::from_millis(50)),
        }
    }
}
