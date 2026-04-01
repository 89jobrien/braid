use anyhow::{Result, bail};
use braid_model::{ContextChunk, ContextSnapshot};
use chrono::Utc;

use crate::types::ContextSource;

pub const DEFAULT_BUDGET: usize = 2000;

pub struct ContextAssembler {
    sources: Vec<Box<dyn ContextSource>>,
    budget: usize,
}

impl ContextAssembler {
    pub fn new(sources: Vec<Box<dyn ContextSource>>) -> Self {
        Self {
            sources,
            budget: DEFAULT_BUDGET,
        }
    }

    pub fn with_budget(mut self, budget: usize) -> Self {
        self.budget = budget;
        self
    }

    pub fn assemble(&self) -> Result<ContextSnapshot> {
        self.assemble_with_prior(None)
    }

    pub fn refresh(&self, prior: Option<ContextSnapshot>) -> Result<ContextSnapshot> {
        self.assemble_with_prior(prior)
    }

    fn assemble_with_prior(&self, _prior: Option<ContextSnapshot>) -> Result<ContextSnapshot> {
        let now = Utc::now();
        let mut all_chunks: Vec<ContextChunk> = Vec::new();

        // Collect from sources, skip failures
        for source in &self.sources {
            match source.fetch() {
                Ok(chunks) => {
                    let window = source.staleness_window();
                    for chunk in chunks {
                        if now.signed_duration_since(chunk.captured_at) <= window {
                            all_chunks.push(chunk);
                        }
                    }
                }
                Err(_) => {
                    // non-fatal: skip this source
                }
            }
        }

        if all_chunks.is_empty() {
            bail!("all context sources failed or returned no chunks");
        }

        let total_tokens: usize = all_chunks.iter().map(|c| c.token_estimate).sum();
        let threshold = self.budget / 2;

        if total_tokens <= threshold {
            // Short session: staleness filter only, no trimming
            return Ok(ContextSnapshot {
                token_estimate: total_tokens,
                chunks: all_chunks,
                summary: None,
                assembled_at: now,
                dropped_chunks: 0,
            });
        }

        // Over threshold: drop oldest-first until under budget
        all_chunks.sort_by_key(|c| c.captured_at);
        let mut kept = Vec::new();
        let mut running_tokens = 0usize;
        let mut dropped = 0usize;
        for chunk in all_chunks.into_iter().rev() {
            if running_tokens + chunk.token_estimate <= self.budget {
                running_tokens += chunk.token_estimate;
                kept.push(chunk);
            } else {
                dropped += 1;
            }
        }
        kept.reverse();

        Ok(ContextSnapshot {
            token_estimate: running_tokens,
            chunks: kept,
            summary: None,
            assembled_at: now,
            dropped_chunks: dropped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::ContextChunk;
    use chrono::{Duration, Utc};

    struct StubSource {
        name: &'static str,
        window: Duration,
        chunks: Vec<ContextChunk>,
    }

    impl crate::types::ContextSource for StubSource {
        fn name(&self) -> &'static str {
            self.name
        }
        fn staleness_window(&self) -> Duration {
            self.window
        }
        fn fetch(&self) -> Result<Vec<ContextChunk>> {
            Ok(self.chunks.clone())
        }
    }

    struct FailingSource;
    impl crate::types::ContextSource for FailingSource {
        fn name(&self) -> &'static str {
            "fail"
        }
        fn staleness_window(&self) -> Duration {
            Duration::hours(1)
        }
        fn fetch(&self) -> Result<Vec<ContextChunk>> {
            anyhow::bail!("source unavailable")
        }
    }

    fn fresh_chunk(source: &'static str, tokens: usize) -> ContextChunk {
        let content = "x".repeat(tokens * 4);
        ContextChunk {
            source: source.to_string(),
            label: "test".to_string(),
            token_estimate: tokens,
            content,
            captured_at: Utc::now(),
        }
    }

    fn stale_chunk(source: &'static str, tokens: usize) -> ContextChunk {
        let content = "x".repeat(tokens * 4);
        ContextChunk {
            source: source.to_string(),
            label: "test".to_string(),
            token_estimate: tokens,
            content,
            captured_at: Utc::now() - Duration::hours(2),
        }
    }

    #[test]
    fn staleness_filter_drops_old_chunks() {
        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks: vec![fresh_chunk("test", 10), stale_chunk("test", 10)],
        };
        let assembler = ContextAssembler::new(vec![Box::new(source)]).with_budget(10000);
        let snap = assembler.assemble().unwrap();
        assert_eq!(snap.chunks.len(), 1);
        assert_eq!(snap.dropped_chunks, 0);
    }

    #[test]
    fn short_session_no_drop() {
        // 10 tokens, budget 2000 — well under 50% threshold (1000)
        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks: vec![fresh_chunk("test", 10)],
        };
        let assembler = ContextAssembler::new(vec![Box::new(source)]);
        let snap = assembler.assemble().unwrap();
        assert_eq!(snap.dropped_chunks, 0);
        assert!(snap.summary.is_none());
        assert_eq!(snap.chunks.len(), 1);
    }

    #[test]
    fn budget_exceeded_drops_oldest() {
        // budget=100, threshold=50; each chunk is 60 tokens → total 120 → over threshold
        // oldest chunk (captured 10s ago) should be dropped first
        let mut chunk_old = fresh_chunk("test", 60);
        chunk_old.captured_at = Utc::now() - Duration::seconds(10);
        let chunk_new = fresh_chunk("test", 60);

        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks: vec![chunk_old, chunk_new],
        };
        let assembler = ContextAssembler::new(vec![Box::new(source)]).with_budget(100);
        let snap = assembler.assemble().unwrap();
        assert_eq!(snap.dropped_chunks, 1);
        assert_eq!(snap.chunks.len(), 1);
        assert!(snap.token_estimate <= 100);
    }

    #[test]
    fn failing_source_is_non_fatal_when_other_source_succeeds() {
        let good = StubSource {
            name: "good",
            window: Duration::hours(1),
            chunks: vec![fresh_chunk("good", 10)],
        };
        let assembler = ContextAssembler::new(vec![Box::new(FailingSource), Box::new(good)]);
        let snap = assembler.assemble().unwrap();
        assert_eq!(snap.chunks.len(), 1);
        assert_eq!(snap.chunks[0].source, "good");
    }

    #[test]
    fn all_sources_failing_returns_error() {
        let assembler = ContextAssembler::new(vec![Box::new(FailingSource)]);
        assert!(assembler.assemble().is_err());
    }

    #[test]
    fn dropped_chunks_always_zero_when_nothing_dropped() {
        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks: vec![fresh_chunk("test", 1)],
        };
        let assembler = ContextAssembler::new(vec![Box::new(source)]);
        let snap = assembler.assemble().unwrap();
        assert_eq!(snap.dropped_chunks, 0);
    }
}
