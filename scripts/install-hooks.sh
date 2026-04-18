#!/usr/bin/env bash
set -euo pipefail

echo "Installing git hooks..."

git config core.hooksPath .githooks

echo "Git hooks installed (core.hooksPath = .githooks)"
echo
echo "Active hooks:"
echo "  - pre-commit: fmt + clippy + test"
echo "  - pre-push:   full test suite"
echo "  - commit-msg: conventional commit format (warning only)"
echo
echo "To bypass hooks (not recommended):"
echo "  git commit --no-verify"
