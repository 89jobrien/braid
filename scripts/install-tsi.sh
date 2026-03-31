#!/usr/bin/env bash
#
# Install TSI command globally
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TSI_SCRIPT="$SCRIPT_DIR/tsi"
INSTALL_DIR="${HOME}/.local/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

# Check if TSI script exists
if [[ ! -f "$TSI_SCRIPT" ]]; then
    log_error "TSI script not found at $TSI_SCRIPT"
    exit 1
fi

# Create install directory if it doesn't exist
if [[ ! -d "$INSTALL_DIR" ]]; then
    log_info "Creating install directory: $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
fi

# Copy or symlink the script
if [[ "$1" == "--symlink" ]] 2>/dev/null; then
    log_info "Creating symlink to TSI script"
    ln -sf "$TSI_SCRIPT" "$INSTALL_DIR/tsi"
else
    log_info "Copying TSI script to $INSTALL_DIR"
    cp "$TSI_SCRIPT" "$INSTALL_DIR/tsi"
    chmod +x "$INSTALL_DIR/tsi"
fi

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    log_warn "WARNING: $INSTALL_DIR is not in your PATH"
    log_warn "Add the following line to your ~/.zshrc or ~/.bashrc:"
    log_warn "    export PATH=\"\$HOME/.local/bin:\$PATH\""
    log_warn ""
    log_warn "Then run: source ~/.zshrc (or ~/.bashrc)"
else
    log_success "TSI command installed successfully!"
    log_info "You can now run 'tsi --help' from anywhere"
fi

# Test the installation
if command -v tsi >/dev/null 2>&1; then
    log_success "Installation verified - TSI command is available"
    echo ""
    tsi help
else
    log_warn "TSI command not yet available in PATH"
    log_info "Make sure $INSTALL_DIR is in your PATH and restart your shell"
fi