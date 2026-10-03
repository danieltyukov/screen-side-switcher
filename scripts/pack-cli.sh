#!/bin/sh
# Packs a built screen-side binary the way a release ships it: the binary,
# LICENSE and README.md at the top of a .tar.gz.
#
#   scripts/pack-cli.sh <binary> <asset-name.tar.gz> [out-dir]
set -eu

bin=$1
asset=$2
out=${3:-dist}
root=$(cd "$(dirname "$0")/.." && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT INT TERM

cp "$bin" "$stage/screen-side"
chmod 755 "$stage/screen-side"
cp "$root/LICENSE" "$root/README.md" "$stage/"
mkdir -p "$out"
tar -czf "$out/$asset" -C "$stage" screen-side LICENSE README.md
echo "$out/$asset"
