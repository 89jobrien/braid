use anyhow::Result;
use braid_model::ContextSnapshot;
use braid_ports::ContextProvider;
use std::sync::Mutex;

use crate::assembler::ContextAssembler;

/// Implements the `ContextProvider` port using `ContextAssembler`.
/// Holds the last snapshot for rolling refresh.
pub struct ContextAssemblerProvider {
    assembler: ContextAssembler,
    last_snapshot: Mutex<Option<ContextSnapshot>>,
}

impl ContextAssemblerProvider {
    pub fn new(assembler: ContextAssembler) -> Self {
        Self {
            assembler,
            last_snapshot: Mutex::new(None),
        }
    }
}

impl ContextProvider for ContextAssemblerProvider {
    fn assemble(&self) -> Result<ContextSnapshot> {
        let snap = self.assembler.assemble()?;
        *self.last_snapshot.lock().expect("lock poisoned") = Some(snap.clone());
        Ok(snap)
    }

    fn refresh(&self) -> Result<ContextSnapshot> {
        let prior = self.last_snapshot.lock().expect("lock poisoned").clone();
        let snap = self.assembler.refresh(prior)?;
        *self.last_snapshot.lock().expect("lock poisoned") = Some(snap.clone());
        Ok(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembler::ContextAssembler;
    use crate::types::ContextSource;
    use anyhow::Result;
    use braid_model::ContextChunk;
    use chrono::Duration;

    struct StubSource(Vec<ContextChunk>);

    impl ContextSource for StubSource {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn staleness_window(&self) -> Duration {
            Duration::hours(1)
        }
        fn fetch(&self) -> Result<Vec<ContextChunk>> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn assembler_provider_implements_context_provider() {
        let chunk = ContextChunk::new("stub", "label", "content with some text here");
        let assembler = ContextAssembler::new(vec![Box::new(StubSource(vec![chunk]))]);
        let provider = ContextAssemblerProvider::new(assembler);
        let snap = provider.assemble().unwrap();
        assert!(!snap.chunks.is_empty());
    }

    #[test]
    fn refresh_uses_prior_snapshot() {
        let chunk = ContextChunk::new("stub", "label", "content with some text here");
        let assembler = ContextAssembler::new(vec![Box::new(StubSource(vec![chunk]))]);
        let provider = ContextAssemblerProvider::new(assembler);
        let _ = provider.assemble().unwrap();
        // refresh should not error when prior exists
        let snap = provider.refresh().unwrap();
        assert_eq!(snap.dropped_chunks, 0);
    }

    // Contract: ContextProvider port impls must always populate dropped_chunks
    #[test]
    fn dropped_chunks_always_present() {
        let chunk = ContextChunk::new("stub", "label", "hello world");
        let assembler = ContextAssembler::new(vec![Box::new(StubSource(vec![chunk]))]);
        let provider = ContextAssemblerProvider::new(assembler);
        let snap = provider.assemble().unwrap();
        let _ = snap.dropped_chunks; // field always present
        assert_eq!(snap.dropped_chunks, 0);
    }
}
