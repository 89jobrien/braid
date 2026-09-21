//! Loads component manifests and exposes them through a filesystem-backed registry.

#![cfg_attr(test, allow(clippy::unwrap_used))]
mod manifest;
mod registry;

pub use manifest::{CommandEntry, ComponentManifest, PromptEntry};
pub use registry::FileSystemRegistry;
