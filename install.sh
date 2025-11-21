#!/usr/bin/env bash
set -euo pipefail

REPO="skolhustick/tengok"
VERSION="${TENGOK_VERSION:-latest}"

err() {
    echo "tengok installer: $*" >&2
    exit 1
}

# Detect OS
case "$(uname -s)" in
    Linux) OS="linux" ;;
    Darwin) OS="macos" ;;
    *) err "unsupported OS: $(uname -s)" ;;
esac

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="arm64" ;;
    *) err "unsupported architecture: $(uname -m)" ;;
esac

# Map OS + ARCH to asset filename
if [[ "$OS" == "macos" ]]; then
    ASSET="tengok-macos-${ARCH}"
else
    ASSET="tengok-linux-${ARCH}"
fi

# Release path
if [[ "$VERSION" == "latest" ]]; then
    RELEASE_PATH="latest/download"
else
    RELEASE_PATH="download/${VERSION}"
fi

URL="https://github.com/${REPO}/releases/${RELEASE_PATH}/${ASSET}"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

echo "Downloading ${ASSET}..."
curl -fsSL "${URL}" -o "${TMPDIR}/tengok" || err "failed to download release asset"
chmod +x "${TMPDIR}/tengok"

echo ""
echo "Where do you want to install tengok?"
echo "1) Only for you   (~/.local/bin)"
echo "2) System-wide    (/usr/local/bin) [requires sudo]"
echo ""

read -rp "Choose option [1/2]: " CHOICE

case "$CHOICE" in
    1)
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
        mv "${TMPDIR}/tengok" "$INSTALL_DIR/tengok" || err "failed to move binary"
        echo "✅ Installed to $INSTALL_DIR/tengok"
        # suggest PATH fix if needed
        if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
            echo ""
            echo "⚠️  $HOME/.local/bin is not on your PATH."
            echo "   Add this to your shell config:"
            echo "     export PATH=\"\$HOME/.local/bin:\$PATH\""
        fi
        ;;
    2)
        INSTALL_DIR="/usr/local/bin"
        sudo mkdir -p "$INSTALL_DIR"
        sudo mv "${TMPDIR}/tengok" "$INSTALL_DIR/tengok" || err "failed to move binary"
        echo "✅ Installed to /usr/local/bin/tengok"
        ;;
    *)
        err "invalid choice"
        ;;
esac

echo ""
echo "Run 'tengok --help' to get started."
