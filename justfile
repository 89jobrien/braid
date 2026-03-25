default:
    cargo check

fmt:
    cargo fmt --all

check:
    cargo check --workspace

clippy:
    cargo clippy --workspace -- -D warnings

test:
    cargo nextest run --workspace
