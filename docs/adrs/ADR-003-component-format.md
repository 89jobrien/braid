# ADR-003: Component Format (Planned)

**Status:** Proposed
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

[[skills]]
name = "extract"
file = "skills/extract.md"

[[prompts]]
name = "system"
file = "prompts/system.txt"
```

A `ComponentRegistry` trait (in `braid-ports`) exposes:

```rust
pub trait ComponentRegistry {
    fn load(&self, path: &Path) -> Result<ComponentHandle>;
    fn get(&self, name: &str) -> Option<&ComponentHandle>;
    fn list(&self) -> Vec<&ComponentHandle>;
}
```

The engine resolves component references by name at session-start time. Unknown
component names are a startup error, not a runtime error.

Loading follows the same pattern as `BraidConfig::load`: read TOML from disk,
deserialize into a typed struct, validate required fields, return `anyhow::Result`.

## Consequences

- **No recompilation**: new components are added by dropping a directory and
  updating a registry config path, not by changing Rust code.
- **TOML-first**: consistent with `braid-bootstrap` config; no new serialization
  formats introduced.
- **Registry as port**: `ComponentRegistry` lives in `braid-ports`, keeping
  `braid-components` as a leaf crate with no reverse dependencies.
- **Startup validation**: missing component files or malformed manifests fail
  fast at load time rather than mid-session.
- **Not yet implemented**: this ADR records the intended shape. Implementation
  begins in Phase 4. Details may change; update status to Accepted when a
  working `ComponentRegistry` impl lands in `braid-components`.

## Alternatives Considered

- **Dynamic plugin (`.so`/`dylib`)**: true runtime extension without restart.
  Rejected for Phase 4 — unsafe, platform-specific, and unnecessary when the
  primary use case is prompt/template packaging rather than native code.
- **JSON manifests**: consistent with event format but less ergonomic for
  human-authored component definitions. TOML matches the established config
  pattern.
- **Embed components in binary**: no filesystem dependency, but eliminates the
  ability to ship components without a rebuild. Defeats the purpose.
