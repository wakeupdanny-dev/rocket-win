#!/usr/bin/env sh
# Downloads the sing-box core binary that the app bundles.
set -e

VER="${SINGBOX_VERSION:-1.14.0}"
URL="https://github.com/SagerNet/sing-box/releases/download/v${VER}/sing-box-${VER}-windows-amd64.zip"
DEST="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"

mkdir -p "$DEST"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading sing-box $VER ..."
curl -fsSL "$URL" -o "$tmp/sing-box.zip"
unzip -oq "$tmp/sing-box.zip" -d "$tmp"
cp "$tmp"/sing-box-*/sing-box.exe "$DEST/sing-box.exe"

echo "-> $DEST/sing-box.exe"
