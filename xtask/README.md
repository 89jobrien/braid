# Braid xtask

`xtask` contains repository-local automation that is too specific to Braid's architecture for a
general-purpose task runner. It is an unpublished binary and is invoked from the workspace root.

## Workspace role

The task enforces dependency direction in the hexagonal workspace. It reads each
`crates/*/Cargo.toml`, extracts direct `braid-*` dependencies, and compares them with a fixed set of
boundary rules. It does not compile crates or inspect Rust imports.

The intended main chain is:

```text
braid-cli -> braid-providers -> braid-engine -> braid-ports -> braid-model
```

Support crates must not depend on `braid-providers` or `braid-cli`.

## Commands

```console
cargo run -p xtask -- check-deps
```

When the local `cargo xtask` alias is installed, the equivalent command is:

```console
cargo xtask check-deps
```

`check-deps` exits successfully when every rule passes. On failure it prints every dependent and
forbidden crate pair, the architectural reason, and a non-zero error.

## Implementation status

The binary currently implements only `check-deps`; unknown or missing task names are errors. The
check currently reports that `braid-tui` depends directly on `braid-providers`, which conflicts
with the support-crate rule encoded in `xtask/src/main.rs`.

## Features

This package has no Cargo feature flags. It is `publish = false` and versioned independently as
`0.0.0` because it is workspace tooling rather than a library product.

## Development and testing

Run the task from the workspace root because it expects `Cargo.toml` and `crates/` in the current
directory:

```console
cargo run -p xtask -- check-deps
just check
just clippy
just test
```

The implementation is in `xtask/src/main.rs`. Add new boundary rules to `RULES`; add a new task by
extending the argument dispatch in `main`.
