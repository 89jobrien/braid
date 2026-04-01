use anyhow::Result;
use chrono::Duration;

// Re-export model types as the canonical source
pub use braid_model::{ContextChunk, ContextSnapshot, ContextSummary};

pub trait ContextSource {
    fn name(&self) -> &'static str;
    fn staleness_window(&self) -> Duration;
    fn fetch(&self) -> Result<Vec<ContextChunk>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn chunk_token_estimate_is_char_count_div_4() {
        let chunk = ContextChunk::new("test", "label", "abcd");
        assert_eq!(chunk.token_estimate, 1);
    }

    #[test]
    fn chunk_token_estimate_rounds_down() {
        let chunk = ContextChunk::new("test", "label", "abc");
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
        let chunk = ContextChunk::new("test", "l", "abcd");
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
