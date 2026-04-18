use std::sync::Arc;

use std::fmt::Write as FmtWrite;

use anyhow::{Result, bail};
use braid_model::{
    ContentPart, ContextChunk, ContextSnapshot, ContextSummary, Message, ProviderRequest, Role,
    estimate_tokens,
};
use braid_ports::Provider;
use chrono::Utc;

use crate::types::ContextSource;

pub const DEFAULT_BUDGET: usize = 2000;

pub struct ContextAssembler {
    sources: Vec<Box<dyn ContextSource>>,
    budget: usize,
    provider: Option<Arc<dyn Provider + Send + Sync>>,
}

impl ContextAssembler {
    pub fn new(sources: Vec<Box<dyn ContextSource>>) -> Self {
        Self {
            sources,
            budget: DEFAULT_BUDGET,
            provider: None,
        }
    }

    #[must_use]
    pub const fn with_budget(mut self, budget: usize) -> Self {
        self.budget = budget;
        self
    }

    #[must_use]
    pub fn with_provider(mut self, provider: Arc<dyn Provider + Send + Sync>) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn assemble(&self) -> Result<ContextSnapshot> {
        self.assemble_with_prior(None)
    }

    pub fn refresh(&self, prior: Option<&ContextSnapshot>) -> Result<ContextSnapshot> {
        self.assemble_with_prior(prior)
    }

    fn assemble_with_prior(&self, prior: Option<&ContextSnapshot>) -> Result<ContextSnapshot> {
        let now = Utc::now();
        let mut all_chunks: Vec<ContextChunk> = Vec::new();

        // Collect from sources, skip failures
        for source in &self.sources {
            if let Ok(chunks) = source.fetch() {
                let window = source.staleness_window();
                for chunk in chunks {
                    if now.signed_duration_since(chunk.captured_at) <= window {
                        all_chunks.push(chunk);
                    }
                }
            }
            // else: non-fatal: skip this source
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

        // Over threshold: try LLM summarization if provider is wired
        if let Some(provider) = &self.provider {
            match Self::summarize(provider.as_ref(), &all_chunks, prior) {
                Ok(summary) => {
                    let token_estimate = summary.token_estimate;
                    return Ok(ContextSnapshot {
                        token_estimate,
                        chunks: vec![],
                        summary: Some(summary),
                        assembled_at: now,
                        dropped_chunks: 0,
                    });
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "context summarization failed; falling back to oldest-first drop"
                    );
                }
            }
        }
        // else: fall through to oldest-first drop

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

    fn summarize(
        provider: &dyn Provider,
        chunks: &[ContextChunk],
        prior: Option<&ContextSnapshot>,
    ) -> Result<ContextSummary> {
        let mut prompt = String::new();

        if let Some(p) = prior
            && let Some(summary) = &p.summary
        {
            prompt.push_str(&summary.content);
            prompt.push('\n');
        }

        prompt.push_str("New context to integrate:\n");
        for chunk in chunks {
            write!(
                prompt,
                "[{}] {}\n{}\n\n",
                chunk.source, chunk.label, chunk.content
            )
            .expect("writing to String is infallible");
        }
        prompt.push_str("Produce a concise summary of the above context in under 400 words.");

        let request = ProviderRequest {
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: prompt }],
            }],
            tools: vec![],
        };

        let response = provider.complete(request)?;

        let content = response
            .message
            .content
            .into_iter()
            .find_map(|part| {
                if let ContentPart::Text { text } = part {
                    Some(text)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let token_estimate = estimate_tokens(&content);

        Ok(ContextSummary {
            content,
            summarized_at: Utc::now(),
            source_chunk_count: chunks.len(),
            token_estimate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_model::ContextChunk;
    use braid_model::{ContentPart, Message, ProviderRequest, ProviderResponse, Role, TokenCount};
    use braid_ports::Provider;
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
        let snap = assembler.assemble().expect("should succeed");
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
        let snap = assembler.assemble().expect("should succeed");
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
        let snap = assembler.assemble().expect("should succeed");
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
        let snap = assembler.assemble().expect("should succeed");
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
        let snap = assembler.assemble().expect("should succeed");
        assert_eq!(snap.dropped_chunks, 0);
    }

    struct MockProvider {
        response: String,
    }

    impl Provider for MockProvider {
        fn complete(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
            Ok(ProviderResponse {
                message: Message {
                    role: Role::Assistant,
                    content: vec![ContentPart::Text {
                        text: self.response.clone(),
                    }],
                },
                token_count: Some(TokenCount {
                    input: 10,
                    output: 20,
                }),
            })
        }
    }

    #[test]
    fn summarization_triggers_when_over_threshold() {
        // budget=100, threshold=50; chunks total 200 tokens → triggers summarization
        let chunks = vec![fresh_chunk("test", 100), fresh_chunk("test", 100)];
        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks,
        };
        let provider = Arc::new(MockProvider {
            response: "this is the summary".to_string(),
        });
        let assembler = ContextAssembler::new(vec![Box::new(source)])
            .with_budget(100)
            .with_provider(provider);
        let snap = assembler.assemble().expect("should succeed");
        assert!(snap.summary.is_some());
        assert_eq!(
            snap.summary.expect("should succeed").content,
            "this is the summary"
        );
        assert!(snap.chunks.is_empty());
    }

    #[test]
    fn summarization_failure_falls_back_to_drop() {
        struct FailProvider;
        impl Provider for FailProvider {
            fn complete(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
                anyhow::bail!("provider unavailable")
            }
        }

        let chunks = vec![fresh_chunk("test", 100), fresh_chunk("test", 100)];
        let source = StubSource {
            name: "test",
            window: Duration::hours(1),
            chunks,
        };
        let provider = Arc::new(FailProvider);
        let assembler = ContextAssembler::new(vec![Box::new(source)])
            .with_budget(100)
            .with_provider(provider);
        // Should not error — falls back to oldest-first drop
        let snap = assembler.assemble().expect("should succeed");
        assert!(snap.summary.is_none());
        assert!(snap.token_estimate <= 100);
    }
}
