#![cfg_attr(test, allow(clippy::unwrap_used))]
pub mod contract;
pub mod executor;
pub mod guards;
pub mod registry;

pub use contract::{Hook, HookContext, HookVerdict};
pub use executor::HookedExecutor;
pub use guards::{DestructiveCommandGuard, FreshnessGuard};
pub use registry::HookRegistry;
