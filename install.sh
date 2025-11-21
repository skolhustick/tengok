#!/usr/bin/env bash
set -euo pipefail

REPO="skolhustick/tengok"
INSTALL_DIR="${TENGOK_INSTALL_DIR:-/usr/local/bin}"
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

# Version handling
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

echo "Making it executable..."
chmod +x "${TMPDIR}/tengok"

echo "Installing to ${INSTALL_DIR}/tengok ..."
mkdir -p "${INSTALL_DIR}" || err "cannot create install dir"
mv "${TMPDIR}/tengok" "${INSTALL_DIR}/tengok" || err "failed to move binary"

echo "✅ Installed at ${INSTALL_DIR}/tengok"
echo "   Run: tengok --help"
