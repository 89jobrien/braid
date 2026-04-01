use anyhow::Result;
use braid_model::ContextChunk;
use chrono::Duration;
use std::process::Command;

use crate::types::ContextSource;

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

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("doob exited with error: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let todos: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap_or_default();

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
