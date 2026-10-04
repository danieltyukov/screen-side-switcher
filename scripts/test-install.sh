#!/bin/sh
# check() evaluates its first argument, so those are single-quoted on purpose.
# shellcheck disable=SC2016
# Builds the CLI, packs it like a release, and runs site/public/install.sh
# against it in a throwaway HOME that holds a Screen Side 1.0 install.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

cargo build --release -q -p screen-side --manifest-path "$root/Cargo.toml"
case "$(uname -s)" in
  Darwin) asset=screen-side-macos-universal.tar.gz ;;
  *)
    case "$(uname -m)" in
      aarch64 | arm64) asset=screen-side-linux-arm64.tar.gz ;;
      *) asset=screen-side-linux-x64.tar.gz ;;
    esac
    ;;
esac
sh "$root/scripts/pack-cli.sh" "$root/target/release/screen-side" "$asset" "$work/release" >/dev/null
(cd "$work/release" && { sha256sum "$asset" 2>/dev/null || shasum -a 256 "$asset"; } >SHA256SUMS)

home="$work/home"
mkdir -p "$home/.local/bin" "$home/.local/share/screen-side-switcher/screenside" \
  "$home/.local/share/applications" "$home/.local/share/icons/hicolor/scalable/apps"
printf '#!/usr/bin/env python3\nimport sys\nsys.path.insert(0, "x")\nfrom screenside.cli import main\n' >"$home/.local/bin/screen-side"
printf '#!/usr/bin/env python3\nfrom screenside.app import main\n' >"$home/.local/bin/screen-side-gui"
printf '[Desktop Entry]\nExec=%s/.local/bin/screen-side-gui\n' "$home" \
  >"$home/.local/share/applications/io.github.danieltyukov.ScreenSide.desktop"
touch "$home/.local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg"
echo keep >"$home/.local/bin/unrelated"

HOME="$home" SCREEN_SIDE_ARCHIVE="$work/release/$asset" SCREEN_SIDE_INSTALL_DIR="$home/.local/bin" \
  sh "$root/site/public/install.sh"

check() {
  if eval "$1"; then
    echo "ok: $2"
  else
    echo "FAIL: $2" >&2
    exit 1
  fi
}
check '"$home/.local/bin/screen-side" --version | grep -q " 2\."' "the new screen-side runs"
check '[ ! -e "$home/.local/share/screen-side-switcher" ]' "the 1.0 library is gone"
check '[ ! -e "$home/.local/bin/screen-side-gui" ]' "the 1.0 app launcher is gone"
check '[ ! -e "$home/.local/share/applications/io.github.danieltyukov.ScreenSide.desktop" ]' "the 1.0 desktop entry is gone"
check '[ ! -e "$home/.local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg" ]' "the 1.0 icon is gone"
check '[ -e "$home/.local/bin/unrelated" ]' "unrelated files stay"

# A tampered archive installs nothing.
echo tampered >>"$work/release/$asset"
if HOME="$home" SCREEN_SIDE_ARCHIVE="$work/release/$asset" SCREEN_SIDE_INSTALL_DIR="$home/other" \
  sh "$root/site/public/install.sh" 2>/dev/null; then
  echo "FAIL: a tampered archive was accepted" >&2
  exit 1
fi
check '[ ! -e "$home/other/screen-side" ]' "a tampered archive installs nothing"
echo "install.sh: all checks passed"
