# ADR-003: Component Format

**Status:** Accepted
**Date:** 2026-07-13
**Deciders:** Joseph O'Brien

## Context

Phase 4 of the braid build plan introduces `braid-components`: a mechanism for
packaging reusable agent capabilities (commands, skills, prompts, templates) as
self-contained units that can be loaded at runtime without recompiling the
binary. The existing `braid-bootstrap` crate establishes the TOML-based config
pattern (`BraidConfig`) and a `load(path)` → deserialize idiom that components
should follow.

## Decision

Components are **manifest-driven directories** containing a `component.toml`
at their root. The manifest declares metadata and references to resource files:

```toml
[component]
name    = "my-component"
version = "0.1.0"

[[commands]]
name = "summarize"
file = "commands/summarize.md"

[[prompts]]
name = "system"
file = "prompts/system.txt"
```

Supported manifest keys: `[component]` (required: `name`, `version`; optional:
`description`), `[[commands]]`, `[[prompts]]`. Each entry has `name` and `file`
(path relative to the component directory). All referenced files are validated at
load time — a missing file is a startup error.

`ComponentRegistry` (in `braid-ports`) and `ComponentInfo` (returned by queries):

```rust
pub struct ComponentInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

pub trait ComponentRegistry {
    fn load(&mut self, path: &Path) -> Result<()>;
    fn get(&self, name: &str) -> Option<ComponentInfo>;
    fn list(&self) -> Vec<ComponentInfo>;
}
```

`FileSystemRegistry` (in `braid-components`) implements `ComponentRegistry` by
scanning a root directory: each immediate subdirectory containing `component.toml`
is loaded. Direct manifest access (for reading file contents) is available via
`FileSystemRegistry::get_manifest(name) -> Option<&ComponentManifest>`.

The built-in `hello-world` component ships under
`crates/braid-components/components/hello-world/` and serves as the reference
implementation.

## Consequences

- **No recompilation**: new components are added by dropping a directory into
  `~/.braid/components/` and restarting braid.
- **TOML-first**: consistent with `braid-bootstrap` config; no new serialization
  formats introduced.
- **Registry as port**: `ComponentRegistry` lives in `braid-ports`, keeping
  `braid-components` as a leaf crate with no reverse dependencies.
- **Startup validation**: missing component files or malformed manifests fail
  fast at load time rather than mid-session.
- **`ComponentInfo` not `ComponentHandle`**: registry queries return owned
  `ComponentInfo` structs (name, version, description). File-content access
  requires the concrete `FileSystemRegistry::get_manifest` method.

## Alternatives Considered

- **Dynamic plugin (`.so`/`dylib`)**: true runtime extension without restart.
  Rejected for Phase 4 — unsafe, platform-specific, and unnecessary when the
  primary use case is prompt/template packaging rather than native code.
- **JSON manifests**: consistent with event format but less ergonomic for
  human-authored component definitions. TOML matches the established config
  pattern.
- **Embed components in binary**: no filesystem dependency, but eliminates the
  ability to ship components without a rebuild. Defeats the purpose.
