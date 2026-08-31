#!/usr/bin/env bash
# Install Screen Side for the current user (default) or system wide (--system).
set -euo pipefail

APP_ID="io.github.danieltyukov.ScreenSide"
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

MODE="user"
[[ ${1:-} == "--system" ]] && MODE="system"

if [[ $MODE == system ]]; then
    PREFIX="/usr/local"
    DATA_DIR="$PREFIX/share"
    SUDO="sudo"
else
    PREFIX="$HOME/.local"
    DATA_DIR="$HOME/.local/share"
    SUDO=""
fi

BIN_DIR="$PREFIX/bin"
LIB_DIR="$DATA_DIR/screen-side-switcher"
APPS_DIR="$DATA_DIR/applications"
ICON_DIR="$DATA_DIR/icons/hicolor/scalable/apps"

missing=()
python3 - <<'PY' 2>/dev/null || missing+=("python3-gi gir1.2-gtk-4.0 gir1.2-adw-1")
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk  # noqa: F401
PY
if ((${#missing[@]})); then
    echo "Missing dependencies. Install them with:" >&2
    echo "  sudo apt install ${missing[*]}" >&2
    exit 1
fi

echo "Installing Screen Side ($MODE) into $PREFIX"

$SUDO install -d "$BIN_DIR" "$LIB_DIR" "$APPS_DIR" "$ICON_DIR"
$SUDO rm -rf "${LIB_DIR:?}/screenside"
$SUDO cp -r "$SRC_DIR/screenside" "$LIB_DIR/screenside"
$SUDO find "$LIB_DIR/screenside" -name '__pycache__' -type d -exec rm -rf {} + 2>/dev/null || true

write_launcher() {
    local path="$1" entry="$2"
    $SUDO tee "$path" >/dev/null <<LAUNCHER
#!/usr/bin/env python3
import sys

sys.path.insert(0, "$LIB_DIR")
from screenside.$entry import main

raise SystemExit(main())
LAUNCHER
    $SUDO chmod 755 "$path"
}

write_launcher "$BIN_DIR/screen-side" cli
write_launcher "$BIN_DIR/screen-side-gui" app

$SUDO cp "$SRC_DIR/data/$APP_ID.svg" "$ICON_DIR/$APP_ID.svg"
$SUDO sed "s|^Exec=.*|Exec=$BIN_DIR/screen-side-gui|" \
    "$SRC_DIR/data/$APP_ID.desktop" | $SUDO tee "$APPS_DIR/$APP_ID.desktop" >/dev/null

command -v update-desktop-database >/dev/null && $SUDO update-desktop-database "$APPS_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && $SUDO gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" 2>/dev/null || true

echo
echo "Installed:"
echo "  $BIN_DIR/screen-side       command line"
echo "  $BIN_DIR/screen-side-gui   graphical app"
echo "  $APPS_DIR/$APP_ID.desktop"
echo
if [[ :$PATH: != *:$BIN_DIR:* ]]; then
    echo "Note: $BIN_DIR is not on your PATH; add it to use 'screen-side' directly."
    echo
fi
echo "'Screen Side' now appears in your applications list."
