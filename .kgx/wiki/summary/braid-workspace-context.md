# Braid Workspace Context

Braid is a Rust 2024 agent platform organized around hexagonal boundaries. `braid-model`
owns canonical data, `braid-ports` owns replaceable contracts, `braid-engine` owns the agent
loop, adapter crates implement policy and infrastructure, and `braid-cli`/`braid-tui` compose
the runtime. The central call path is frontend -> engine -> context/planner/provider/tool
executor/event sink, with `HookedExecutor` wrapping tool dispatch and `SessionStore` persisting
events. Evidence: [[Braid]], [[Braid Engine]], [[Braid Ports]], and
`.ctx/godmode/memory-bank/system-patterns.md`.
