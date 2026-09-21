//! Built-in hook policies for destructive commands and context freshness.

pub mod destructive;
pub mod freshness;

pub use destructive::DestructiveCommandGuard;
pub use freshness::FreshnessGuard;
