//! Context collection, budgeting, summarization, and refresh support for Braid sessions.

#![cfg_attr(test, allow(clippy::unwrap_used))]
pub mod assembler;
pub mod provider;
pub mod sources;
pub mod tool;
pub mod types;

pub use assembler::{ContextAssembler, DEFAULT_BUDGET};
pub use provider::ContextAssemblerProvider;
pub use sources::{DoobSource, RepoSource};
pub use tool::RefreshContextTool;
