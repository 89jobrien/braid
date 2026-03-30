#!/usr/bin/env bash
# Install git hooks for Braid
set -euo pipefail

echo "Installing git hooks..."

# ============================================================================
# Pre-commit hook
# ============================================================================
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/sh
# Braid pre-commit hook
T0=$(date +%s)
echo "[$(date '+%H:%M:%S')] pre-commit: start"
just pre-commit
CODE=$?
T1=$(date +%s)
echo "[$(date '+%H:%M:%S')] pre-commit: done in $((T1 - T0))s"
exit $CODE
EOF

chmod +x .git/hooks/pre-commit

# ============================================================================
# Pre-push hook
# ============================================================================
cat > .git/hooks/pre-push << 'EOF'
#!/bin/sh
# Braid pre-push hook
T0=$(date +%s)
echo "[$(date '+%H:%M:%S')] pre-push: start"
just test
CODE=$?
T1=$(date +%s)
echo "[$(date '+%H:%M:%S')] pre-push: done in $((T1 - T0))s"
exit $CODE
EOF

chmod +x .git/hooks/pre-push

# ============================================================================
# Commit-msg hook (enforce conventional commits - warning only)
# ============================================================================
cat > .git/hooks/commit-msg << 'EOF'
#!/bin/sh
commit_msg_file=$1
commit_msg=$(cat "$commit_msg_file")

# Allow merge commits and reverts
if echo "$commit_msg" | grep -qE "^(Merge|Revert)"; then
    exit 0
fi

# Check format (warn only)
if ! echo "$commit_msg" | grep -qE "^(feat|fix|docs|style|refactor|test|chore|perf|ci|build)(\(.+\))?: .+"; then
    echo "Warning: Commit message doesn't follow conventional format"
    echo "Recommended: type(scope): subject"
    echo "Types: feat, fix, docs, style, refactor, test, chore"
    echo
fi

exit 0
EOF

chmod +x .git/hooks/commit-msg

echo "Git hooks installed successfully!"
echo
echo "Installed hooks:"
echo "  - pre-commit: fmt + clippy + test"
echo "  - pre-push:   full test suite"
echo "  - commit-msg: conventional commit format (warning only)"
echo
echo "To bypass hooks (not recommended):"
echo "  git commit --no-verify"
