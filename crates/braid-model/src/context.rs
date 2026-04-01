use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    pub source: String,
    pub label: String,
    pub content: String,
    pub captured_at: DateTime<Utc>,
    pub token_estimate: usize,
}

impl ContextChunk {
    pub fn new(source: &'static str, label: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let token_estimate = content.len() / 4;
        Self {
            source: source.into(),
            label: label.into(),
            content,
            captured_at: chrono::Utc::now(),
            token_estimate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub content: String,
    pub summarized_at: DateTime<Utc>,
    pub source_chunk_count: usize,
    pub token_estimate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub chunks: Vec<ContextChunk>,
    pub summary: Option<ContextSummary>,
    pub assembled_at: DateTime<Utc>,
    pub token_estimate: usize,
    pub dropped_chunks: usize,
}

impl ContextSnapshot {
    pub fn render(&self) -> String {
        let mut parts = Vec::new();
        if let Some(summary) = &self.summary {
            parts.push(format!("## Context Summary\n{}", summary.content));
        }
        for chunk in &self.chunks {
            parts.push(format!(
                "## {}: {}\n{}",
                chunk.source, chunk.label, chunk.content
            ));
        }
        parts.join("\n\n")
    }

    pub fn total_tokens(&self) -> usize {
        let summary_tokens = self.summary.as_ref().map(|s| s.token_estimate).unwrap_or(0);
        let chunk_tokens: usize = self.chunks.iter().map(|c| c.token_estimate).sum();
        summary_tokens + chunk_tokens
    }
}
