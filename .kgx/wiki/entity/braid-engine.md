# Braid Engine

`braid-engine` implements the generic agent loop. It depends on [[Braid Ports]] and
`braid-model`, and calls context, planner, provider, tool executor, and event sink contracts
(`crates/braid-engine/src/engine.rs:40-211`; `crates/braid-engine/Cargo.toml:12-16`).
