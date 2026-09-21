//! Built-in context sources for task and repository state.

pub mod doob;
pub mod repo;

pub use doob::DoobSource;
pub use repo::RepoSource;
