# braid-components

`braid-components` loads reusable command and prompt components from directories on disk. It parses
`component.toml`, validates referenced files, exposes their contents, and implements the
`braid_ports::ComponentRegistry` port.

## Workspace role

This crate is the filesystem adapter for Braid's component registry. Frontends can discover
components through the generic port while using `ComponentManifest` when they need command or prompt
contents.

Expected layout:

```text
components/
└── example/
    ├── component.toml
    ├── commands/
    │   └── run.md
    └── prompts/
        └── system.txt
```

## Manifest format

```toml
[component]
name = "example"
version = "0.1.0"
description = "Example component"

[[commands]]
name = "run"
file = "commands/run.md"

[[prompts]]
name = "system"
file = "prompts/system.txt"
```

Every listed file must exist when the manifest is loaded. Paths are resolved relative to the
component directory.

## Key APIs

| API | Purpose |
| --- | --- |
| `ComponentManifest::load` | Parse and validate one component directory |
| `ComponentManifest::read_command` | Read a named command file |
| `ComponentManifest::read_prompt` | Read a named prompt file |
| `FileSystemRegistry::from_dir` | Scan immediate child directories containing `component.toml` |
| `FileSystemRegistry::get_manifest` | Access full loaded manifest data |
| `ComponentRegistry` implementation | Dynamically load, get, and list component metadata |

```rust
use braid_components::FileSystemRegistry;
use braid_ports::ComponentRegistry;

let registry = FileSystemRegistry::from_dir(std::path::Path::new("./components"))?;
for component in registry.list() {
    println!("{} {}", component.name, component.version);
}
# Ok::<(), anyhow::Error>(())
```

## Implementation status

Manifest parsing, validation, sorted listing, dynamic loading, and command/prompt reads are
implemented. The crate does not execute commands, render prompts, install components, recurse below
immediate component directories, or enforce semantic-version syntax.

## Features

This crate has no Cargo feature flags.

## Development and testing

Tests build complete and malformed component trees in temporary directories.

```console
cargo nextest run -p braid-components
cargo clippy -p braid-components -- -D warnings
cargo check -p braid-components
```
