#!/bin/sh
# Installs the screen-side command on macOS and Linux.
#
#   curl -LsSf https://danieltyukov.github.io/screen-side-switcher/install.sh | sh
#
# It downloads the archive for this system from the latest GitHub release,
# checks it against the release's SHA256SUMS, and puts screen-side in
# ~/.local/bin. A Screen Side 1.0 install in ~/.local is removed first.
#
# SCREEN_SIDE_INSTALL_DIR  where to put it (default ~/.local/bin)
# SCREEN_SIDE_VERSION      a version such as 2.0.0 (default: the latest)
# SCREEN_SIDE_ARCHIVE      a local archive to install instead of downloading
set -eu

REPO="danieltyukov/screen-side-switcher"
VERSION="${SCREEN_SIDE_VERSION:-latest}"
DEST="${SCREEN_SIDE_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() {
  printf 'screen-side install: %s\n' "$*" >&2
  exit 1
}

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Linux)
    case "$arch" in
      x86_64 | amd64) asset=screen-side-linux-x64.tar.gz ;;
      aarch64 | arm64) asset=screen-side-linux-arm64.tar.gz ;;
      *) fail "there is no build for Linux on $arch yet. Build it with: cargo install --git https://github.com/$REPO screen-side" ;;
    esac
    app="the .deb, .rpm or AppImage at https://github.com/$REPO/releases/latest"
    ;;
  Darwin)
    asset=screen-side-macos-universal.tar.gz
    app="ScreenSide_universal.dmg at https://github.com/$REPO/releases/latest"
    ;;
  *) fail "this script is for macOS and Linux. On Windows, use install.ps1." ;;
esac

if [ "$VERSION" = latest ]; then
  base="https://github.com/$REPO/releases/latest/download"
else
  base="https://github.com/$REPO/releases/download/v${VERSION#v}"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$1" -O "$2"
  else
    fail "curl or wget is needed to download Screen Side."
  fi
}

if [ -n "${SCREEN_SIDE_ARCHIVE:-}" ]; then
  name=$(basename "$SCREEN_SIDE_ARCHIVE")
  cp "$SCREEN_SIDE_ARCHIVE" "$tmp/$name"
  sums="$(dirname "$SCREEN_SIDE_ARCHIVE")/SHA256SUMS"
  [ -f "$sums" ] && cp "$sums" "$tmp/SHA256SUMS"
else
  name=$asset
  say "Downloading $name"
  fetch "$base/$name" "$tmp/$name" || fail "could not download $base/$name"
  fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "could not download the checksums for $name"
fi

# The archive must match the release's checksum before anything is installed.
if [ -f "$tmp/SHA256SUMS" ]; then
  expected=$(awk -v n="$name" '{ f = $2; sub(/^\*/, "", f); if (f == n) print $1 }' "$tmp/SHA256SUMS")
  [ -n "$expected" ] || fail "SHA256SUMS has no line for $name."
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$name" | awk '{ print $1 }')
  else
    actual=$(shasum -a 256 "$tmp/$name" | awk '{ print $1 }')
  fi
  [ "$expected" = "$actual" ] || fail "the checksum of $name does not match the release. Nothing was installed."
fi

mkdir -p "$tmp/x"
tar -xzf "$tmp/$name" -C "$tmp/x" || fail "$name is not a readable archive."
[ -f "$tmp/x/screen-side" ] || fail "$name does not contain screen-side."

# Screen Side 1.0 lived in ~/.local as a Python package. Each file is removed
# only when it is recognisably 1.0's.
legacy="$HOME/.local/share/screen-side-switcher"
if [ -d "$legacy/screenside" ]; then
  rm -rf "$legacy"
  say "Removed Screen Side 1.0 from $legacy"
fi
for launcher in "$HOME/.local/bin/screen-side-gui" "$HOME/.local/bin/screen-side"; do
  if [ -f "$launcher" ] && grep -q 'from screenside\.' "$launcher" 2>/dev/null; then
    rm -f "$launcher"
    say "Removed the 1.0 launcher $launcher"
  fi
done
entry="$HOME/.local/share/applications/io.github.danieltyukov.ScreenSide.desktop"
if [ -f "$entry" ] && grep -q '^Exec=.*/\.local/bin/screen-side-gui' "$entry"; then
  rm -f "$entry" "$HOME/.local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg"
  say "Removed the 1.0 menu entry"
fi

mkdir -p "$DEST"
cp "$tmp/x/screen-side" "$DEST/screen-side"
chmod 755 "$DEST/screen-side"
say "Installed $("$DEST/screen-side" --version) to $DEST/screen-side"

case ":$PATH:" in
  *":$DEST:"*) ;;
  *) say "$DEST is not on your PATH. Add it, for example: echo 'export PATH=\"$DEST:\$PATH\"' >> ~/.profile" ;;
esac
say "Try: screen-side status"
say "The app with the window and tray icon is $app"
