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

pre-commit:
    cargo fmt --all --check
    cargo clippy --workspace -- -D warnings
    cargo nextest run --workspace

install-hooks:
    bash scripts/install-hooks.sh
