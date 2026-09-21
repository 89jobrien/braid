# Braid Ports

`braid-ports` defines the replaceable contracts used by [[Braid Engine]] and outer adapters,
including provider, tool execution, redaction, event, context, hook, storage, and component
ports (`crates/braid-ports/src/lib.rs:11-175`). It depends only on `braid-model` internally
(`crates/braid-ports/Cargo.toml:12-15`).
