default:
    just --list

fmt:
    cargo fmt --all

check:
    cargo check --workspace

clippy:
    cargo clippy --workspace

test:
    cargo nextest run --workspace

pre-commit:
    cargo fmt --all --check
    cargo build --release --workspace
    cargo clippy --workspace
    cargo nextest run --workspace

install-hooks:
    bash scripts/install-hooks.sh
