#![allow(clippy::unwrap_used)]
//! Integration tests for braid-context.
//! These tests shell out to real git and doob — run with:
//!   cargo test -p braid-context -- --include-ignored

use braid_context::assembler::{ContextAssembler, DEFAULT_BUDGET};
use braid_context::sources::{DoobSource, RepoSource};

#[test]
#[ignore = "requires git repo in current directory"]
fn repo_source_returns_snapshot_in_braid_repo() {
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| std::path::PathBuf::from(d).join("../.."))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    let source = RepoSource::with_root(repo_root);
    let assembler = ContextAssembler::new(vec![Box::new(source)]);
    let snap = assembler.assemble().unwrap();
    // braid repo always has commits — snapshot has reasonable token estimate
    assert!(snap.token_estimate <= DEFAULT_BUDGET * 10);
    // dropped_chunks field always present
    let _ = snap.dropped_chunks;
}

#[test]
#[ignore = "requires doob installed and project configured"]
fn doob_source_returns_snapshot_without_panic() {
    let source = DoobSource::new();
    let assembler = ContextAssembler::new(vec![Box::new(source)]);
    // Should not panic — either succeeds or fails with acceptable error
    match assembler.assemble() {
        Ok(snap) => {
            assert!(snap.token_estimate <= DEFAULT_BUDGET * 10);
        }
        Err(e) => {
            assert!(e.to_string().contains("all context sources failed"));
        }
    }
}
