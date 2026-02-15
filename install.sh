#!/usr/bin/env bash
set -euo pipefail

REPO="Fabianstw/clitodo"
BIN="todo"             
VERSION="${VERSION:-latest}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  os="unknown-linux-gnu" ;;
  Darwin) os="apple-darwin" ;;
  *) echo "Unsupported OS: $os" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
esac

target="${arch}-${os}"
asset="${BIN}-${target}.tar.gz"
sha_asset="${asset}.sha256"

if [[ "$VERSION" == "latest" ]]; then
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
  sha_url="https://github.com/${REPO}/releases/latest/download/${sha_asset}"
else
  url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
  sha_url="https://github.com/${REPO}/releases/download/${VERSION}/${sha_asset}"
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading $url"
curl -fsSL "$url" -o "$tmpdir/$asset"
curl -fsSL "$sha_url" -o "$tmpdir/$sha_asset"

cd "$tmpdir"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 -c "$sha_asset"
elif command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "$sha_asset"
else
  echo "No sha256 checker found (shasum/sha256sum)" >&2
  exit 1
fi

tar -xzf "$asset"

install_dir="${INSTALL_DIR:-/usr/local/bin}"
if [[ ! -w "$install_dir" ]]; then
  echo "Need sudo to install into $install_dir"
  sudo install -m 755 "$BIN" "$install_dir/$BIN"
else
  install -m 755 "$BIN" "$install_dir/$BIN"
fi

echo "Installed $BIN to $install_dir/$BIN"
echo "Try: $BIN --help"
