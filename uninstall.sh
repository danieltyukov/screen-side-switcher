#!/usr/bin/env bash
# Remove Screen Side from the user prefix (default) or the system one (--system).
set -euo pipefail

APP_ID="io.github.danieltyukov.ScreenSide"
MODE="user"
[[ ${1:-} == "--system" ]] && MODE="system"

if [[ $MODE == system ]]; then
    PREFIX="/usr/local"; DATA_DIR="$PREFIX/share"; SUDO="sudo"
else
    PREFIX="$HOME/.local"; DATA_DIR="$HOME/.local/share"; SUDO=""
fi

$SUDO rm -rf "$DATA_DIR/screen-side-switcher"
$SUDO rm -f "$PREFIX/bin/screen-side" "$PREFIX/bin/screen-side-gui"
$SUDO rm -f "$DATA_DIR/applications/$APP_ID.desktop"
$SUDO rm -f "$DATA_DIR/icons/hicolor/scalable/apps/$APP_ID.svg"

command -v update-desktop-database >/dev/null && $SUDO update-desktop-database "$DATA_DIR/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && $SUDO gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" 2>/dev/null || true

echo "Screen Side removed. Your monitor arrangement is unchanged."
