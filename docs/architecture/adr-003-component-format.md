# ADR-003: Component Format

**Date:** 2026-04-01
**Status:** Accepted (deferred to Phase 4)

## Context

Braid will eventually support loadable bundles of commands, skills, prompts, and templates (`braid-components`). The format must be defined before implementation begins to prevent divergence between the manifest schema and the loader.

## Decision

Components are packaged as directories containing a `braid-component.toml` manifest:

```toml
[component]
name    = "my-component"
version = "0.1.0"
kind    = "skill"           # skill | command | prompt | template

[skill]
entry   = "skill.md"        # relative path to the skill definition file

[command]
entry   = "cmd.nu"          # relative path to the command script

[prompt]
entry   = "prompt.md"

[template]
entry   = "template.md"
vars    = ["project", "language"]   # required substitution variables
```

Key invariants:

- **One manifest, one kind.** A component declares exactly one kind. Bundles (multiple components) are directories containing multiple component subdirectories.
- **Entry files are relative paths.** The loader resolves them against the manifest directory. No absolute paths.
- **Only components with a runtime consumer are supported.** `skill` components are loaded by the engine's skill registry. `command` components are registered as CLI subcommands. `prompt` and `template` components are injected at session start or on demand. If a kind has no consumer, it is not added to the format.
- **`braid-components` owns loading; `braid-core` owns consumption.** The loader produces a `ComponentManifest` struct (defined in `braid-model`). Core and CLI consume it — they do not parse TOML directly.
- **Version field is informational only** until a registry/distribution layer exists.

## Consequences

- Phase 4 implementation of `braid-components` must produce a `ComponentManifest` type in `braid-model` and a loader in `braid-components`.
- The engine and CLI depend on `ComponentManifest`, not on TOML parsing.
- Adding a new kind requires: a new `kind` value in the manifest, a new consumer in the appropriate crate, and a test fixture.
- This format is intentionally minimal. Warehouse sprawl (arbitrary metadata, nested dependencies, registry URLs) is explicitly out of scope for Phase 4.
