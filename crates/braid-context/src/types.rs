use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
pub struct ContextChunk {
    pub source: &'static str,
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
            source,
            label: label.into(),
            content,
            captured_at: Utc::now(),
            token_estimate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextSummary {
    pub content: String,
    pub summarized_at: DateTime<Utc>,
    pub source_chunk_count: usize,
    pub token_estimate: usize,
}

#[derive(Debug, Clone)]
pub struct ContextSnapshot {
    pub chunks: Vec<ContextChunk>,
    pub summary: Option<ContextSummary>,
    pub assembled_at: DateTime<Utc>,
    pub token_estimate: usize,
    pub dropped_chunks: usize,
}

impl ContextSnapshot {
    /// Render as a system message prefix string for injection into ProviderRequest.
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

    /// Total token estimate: summary + live chunks.
    pub fn total_tokens(&self) -> usize {
        let summary_tokens = self.summary.as_ref().map(|s| s.token_estimate).unwrap_or(0);
        let chunk_tokens: usize = self.chunks.iter().map(|c| c.token_estimate).sum();
        summary_tokens + chunk_tokens
    }
}

pub trait ContextSource {
    fn name(&self) -> &'static str;
    fn staleness_window(&self) -> Duration;
    fn fetch(&self) -> Result<Vec<ContextChunk>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_token_estimate_is_char_count_div_4() {
        let chunk = ContextChunk::new("test", "label", "abcd"); // 4 chars
        assert_eq!(chunk.token_estimate, 1);
    }

    #[test]
    fn chunk_token_estimate_rounds_down() {
        let chunk = ContextChunk::new("test", "label", "abc"); // 3 chars
        assert_eq!(chunk.token_estimate, 0);
    }

    #[test]
    fn snapshot_render_includes_chunk_content() {
        let chunk = ContextChunk::new("repo", "recent changes", "diff output here");
        let snapshot = ContextSnapshot {
            token_estimate: chunk.token_estimate,
            chunks: vec![chunk],
            summary: None,
            assembled_at: Utc::now(),
            dropped_chunks: 0,
        };
        let rendered = snapshot.render();
        assert!(rendered.contains("repo"));
        assert!(rendered.contains("recent changes"));
        assert!(rendered.contains("diff output here"));
    }

    #[test]
    fn snapshot_render_includes_summary_when_present() {
        let summary = ContextSummary {
            content: "summary text".to_string(),
            summarized_at: Utc::now(),
            source_chunk_count: 3,
            token_estimate: 3,
        };
        let snapshot = ContextSnapshot {
            chunks: vec![],
            summary: Some(summary),
            assembled_at: Utc::now(),
            token_estimate: 3,
            dropped_chunks: 0,
        };
        assert!(snapshot.render().contains("summary text"));
    }

    #[test]
    fn snapshot_total_tokens_sums_chunks_and_summary() {
        let chunk = ContextChunk::new("test", "l", "abcd"); // 1 token
        let summary = ContextSummary {
            content: "s".to_string(),
            summarized_at: Utc::now(),
            source_chunk_count: 1,
            token_estimate: 5,
        };
        let snapshot = ContextSnapshot {
            token_estimate: 6,
            chunks: vec![chunk],
            summary: Some(summary),
            assembled_at: Utc::now(),
            dropped_chunks: 0,
        };
        assert_eq!(snapshot.total_tokens(), 6);
    }
}
