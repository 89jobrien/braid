#![cfg_attr(test, allow(clippy::unwrap_used))]
mod manifest;
mod registry;

pub use manifest::{CommandEntry, ComponentManifest, PromptEntry};
pub use registry::FileSystemRegistry;
