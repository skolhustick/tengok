#!/usr/bin/env bash
set -e

BIN_NAME="tengok"
DIST_DIR="dist"

echo "==> Cleaning old dist folder"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

echo ""
echo "==> Building macOS (native)"
cargo build --release
cp "target/release/$BIN_NAME" "$DIST_DIR/${BIN_NAME}-macos"

echo ""
echo "==> Building Linux x86_64 (musl)"
cross build --release --target x86_64-unknown-linux-musl
cp "target/x86_64-unknown-linux-musl/release/$BIN_NAME" \
   "$DIST_DIR/${BIN_NAME}-linux-x86_64"

echo ""
echo "==> Building Linux ARM64 (musl)"
cross build --release --target aarch64-unknown-linux-musl
cp "target/aarch64-unknown-linux-musl/release/$BIN_NAME" \
   "$DIST_DIR/${BIN_NAME}-linux-arm64"

echo ""
echo "==> Build complete!"
echo ""
echo "Files created in $DIST_DIR/:"
ls -lh "$DIST_DIR"
