#!/usr/bin/env bash
set -euo pipefail

REPO="skolhustick/tengok"
VERSION="${TENGOK_VERSION:-latest}"
FORCE=0
MODE="interactive"
INSTALL_DIR=""

err() {
    echo "tengok installer: $*" >&2
    exit 1
}

info() {
    echo "▶ $*"
}

# Read input safely even in `curl | bash`
read_input() {
    local prompt="$1"
    local var
    read -rp "$prompt" var < /dev/tty
    echo "$var"
}

###############################################################################
# Parse Flags
###############################################################################
while [[ $# -gt 0 ]]; do
    case "$1" in
        --global) MODE="global" ;;
        --local) MODE="local" ;;
        --force) FORCE=1 ;;
        *) err "Unknown option: $1" ;;
    esac
    shift
done

###############################################################################
# Detect OS
###############################################################################
case "$(uname -s)" in
    Linux) OS="linux" ;;
    Darwin) OS="macos" ;;
    *) err "unsupported OS: $(uname -s)" ;;
esac

###############################################################################
# Detect Architecture
###############################################################################
case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="arm64" ;;
    *) err "unsupported architecture: $(uname -m)" ;;
esac

###############################################################################
# Determine asset file
###############################################################################
if [[ "$OS" == "macos" ]]; then
    ASSET="tengok-macos-${ARCH}"
else
    ASSET="tengok-linux-${ARCH}"
fi

###############################################################################
# Release URL
###############################################################################
if [[ "$VERSION" == "latest" ]]; then
    RELEASE_PATH="latest/download"
else
    RELEASE_PATH="download/${VERSION}"
fi

URL="https://github.com/${REPO}/releases/${RELEASE_PATH}/${ASSET}"

###############################################################################
# Ask where to install (only if interactive)
###############################################################################
choose_install_mode() {
    while true; do
        echo ""
        echo "Where do you want to install tengok?"
        echo "1) Only for you   (~/.local/bin)"
        echo "2) System-wide    (/usr/local/bin) [requires sudo]"
        echo ""

        CHOICE=$(read_input "Choose option [1/2]: ")

        case "$CHOICE" in
            1) INSTALL_DIR="$HOME/.local/bin"; break ;;
            2) INSTALL_DIR="/usr/local/bin"; break ;;
            *) echo "Invalid choice. Please enter 1 or 2." ;;
        esac
    done
}

if [[ "$MODE" == "interactive" ]]; then
    choose_install_mode
elif [[ "$MODE" == "global" ]]; then
    INSTALL_DIR="/usr/local/bin"
elif [[ "$MODE" == "local" ]]; then
    INSTALL_DIR="$HOME/.local/bin"
fi

###############################################################################
# Download binary
###############################################################################
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

info "Downloading $ASSET..."
curl -fsSL "$URL" -o "$TMPDIR/tengok" || err "Failed to download $ASSET"
chmod +x "$TMPDIR/tengok"

TARGET="${INSTALL_DIR}/tengok"

###############################################################################
# Overwrite confirmation
###############################################################################
if [[ -f "$TARGET" && $FORCE -ne 1 ]]; then
    echo ""
    echo "⚠️  A tengok binary already exists at:"
    echo "   $TARGET"
    echo ""
    OVERWRITE=$(read_input "Overwrite it? [y/N]: ")
    case "$OVERWRITE" in
        y|Y) ;;
        *) err "Installation cancelled." ;;
    esac
fi

###############################################################################
# Install binary
###############################################################################
info "Installing to $INSTALL_DIR ..."

mkdir -p "$INSTALL_DIR"

if [[ "$INSTALL_DIR" == "/usr/local/bin" ]]; then
    sudo mv "$TMPDIR/tengok" "$TARGET"
else
    mv "$TMPDIR/tengok" "$TARGET"
fi

###############################################################################
# PATH check
###############################################################################
if ! command -v tengok >/dev/null 2>&1; then
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo "⚠️  $INSTALL_DIR is not on your PATH."
        echo "   Add this line to your shell config:"
        echo "     export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
fi

echo ""
echo "✅ Installed successfully!"
echo "   Run: tengok --help"
