#!/usr/bin/env bash
set -euo pipefail

REPO="skolhustick/tengok"
INSTALL_DIR="${TENGOK_INSTALL_DIR:-/usr/local/bin}"
VERSION="${TENGOK_VERSION:-latest}"

function err() {
    echo "tengok installer: $*" >&2
    exit 1
}

case "$(uname -s)" in
    Linux) OS="linux" ;;
    Darwin) OS="darwin" ;;
    *) err "unsupported OS: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="arm64" ;;
    *) err "unsupported architecture: $(uname -m)" ;;
esac

ASSET="tengok-${OS}-${ARCH}.tar.gz"
if [[ "${VERSION}" == "latest" ]]; then
    RELEASE_PATH="latest/download"
else
    RELEASE_PATH="download/${VERSION}"
fi

URL="https://github.com/${REPO}/releases/${RELEASE_PATH}/${ASSET}"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

echo "Downloading ${ASSET} from ${URL}..."
curl -fsSL "${URL}" -o "${TMPDIR}/${ASSET}" || err "failed to download release asset"

echo "Extracting archive..."
tar -xzf "${TMPDIR}/${ASSET}" -C "${TMPDIR}" || err "failed to extract archive"

BIN_PATH="$(find "${TMPDIR}" -maxdepth 2 -type f -name 'tengok' -print -quit)"
[[ -n "${BIN_PATH}" ]] || err "binary 'tengok' not found inside archive"
chmod +x "${BIN_PATH}"

echo "Installing to ${INSTALL_DIR} (requires write permission)..."
mkdir -p "${INSTALL_DIR}" || err "cannot create install dir"
mv "${BIN_PATH}" "${INSTALL_DIR}/tengok" || err "failed to move binary"

echo "✅ Installed ${INSTALL_DIR}/tengok"
echo "   Run 'tengok --help' or 'tengok --plain /path/to/dir' to get started."

