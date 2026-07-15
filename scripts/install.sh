#!/usr/bin/env bash
set -euo pipefail

PROJECT_NAME="rvcs"
INSTALL_DIR="/usr/local/bin"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { printf "${GREEN}✓${NC} %s\n" "$1"; }
warn()  { printf "${YELLOW}⚠${NC} %s\n" "$1"; }
err()   { printf "${RED}✗${NC} %s\n" "$1"; exit 1; }

# Detect cargo even under sudo (which strips PATH)
find_cargo() {
    command -v cargo 2>/dev/null && return
    for dir in "$HOME/.cargo/bin" "$HOME/.local/share/cargo/bin" /usr/local/cargo/bin /usr/share/cargo/bin; do
        if [[ -x "$dir/cargo" ]]; then
            export PATH="$dir:$PATH"
            return
        fi
    done
    err "cargo is not found. Install Rust first: https://rustup.rs"
}

find_cargo

# Parse flags
BUILD_PROFILE="release"
CUSTOM_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_PROFILE="debug"
            shift
            ;;
        --dir)
            CUSTOM_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--debug] [--dir <path>]"
            echo ""
            echo "  --debug      Build in debug mode (default: release)"
            echo "  --dir <path> Install to a custom directory instead of $INSTALL_DIR"
            echo "  --help, -h   Show this help message"
            exit 0
            ;;
        *)
            err "Unknown option: $1 (use --help for usage)"
            ;;
    esac
done

if [[ -n "$CUSTOM_DIR" ]]; then
    INSTALL_DIR="$CUSTOM_DIR"
fi

# Build as the invoking user (non-root) to avoid running cargo as root
if [[ $EUID -eq 0 ]]; then
    ORIGINAL_USER=$(logname 2>/dev/null || echo "$SUDO_USER")
    if [[ -n "${ORIGINAL_USER:-}" ]]; then
        info "Running build as $ORIGINAL_USER (not root)..."
        sudo -u "$ORIGINAL_USER" bash -lc \
            "cargo build --profile '$BUILD_PROFILE' --manifest-path '$PROJECT_DIR/Cargo.toml'"
    else
        info "Running build as root..."
        cargo build --profile "$BUILD_PROFILE" --manifest-path "$PROJECT_DIR/Cargo.toml"
    fi
else
    info "Building $PROJECT_NAME ($BUILD_PROFILE profile)..."
    cargo build --profile "$BUILD_PROFILE" --manifest-path "$PROJECT_DIR/Cargo.toml"
fi

BINARY="$PROJECT_DIR/target/$BUILD_PROFILE/$PROJECT_NAME"
if [[ ! -f "$BINARY" ]]; then
    err "Build succeeded but binary not found at $BINARY"
fi

mkdir -p "$INSTALL_DIR"
cp "$BINARY" "$INSTALL_DIR/$PROJECT_NAME"
chmod 755 "$INSTALL_DIR/$PROJECT_NAME"

info "Installed $PROJECT_NAME to $INSTALL_DIR/$PROJECT_NAME"

if "$INSTALL_DIR/$PROJECT_NAME" --version >/dev/null 2>&1; then
    INSTALLED_VERSION=$("$INSTALL_DIR/$PROJECT_NAME" --version 2>&1)
    info "Verified: $INSTALLED_VERSION"
else
    warn "Installation path '$INSTALL_DIR' may not be in your PATH"
    warn "Add it: export PATH=\"$INSTALL_DIR:\$PATH\""
fi

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR" >/dev/null 2>&1; then
    [[ "$INSTALL_DIR" != "/usr/local/bin" ]] && \
        warn "$INSTALL_DIR is not in your PATH, add with: export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
info "Installation complete. Run 'rvcs --help' to get started."
