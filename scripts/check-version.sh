#!/bin/sh
# Checks that every place that carries the version agrees, and, given a tag,
# that the tag names that version.
#
#   scripts/check-version.sh [v2.0.0]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
json_version() { sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$1" | head -n 1; }

cargo=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)
app=$(json_version "$root/app/package.json")
site=$(json_version "$root/site/package.json")
tauri=$(json_version "$root/app/src-tauri/tauri.conf.json")

status=0
for pair in "app/package.json:$app" "site/package.json:$site" "app/src-tauri/tauri.conf.json:$tauri"; do
  file=${pair%%:*}
  version=${pair#*:}
  if [ "$version" != "$cargo" ]; then
    echo "$file says $version but Cargo.toml says $cargo" >&2
    status=1
  fi
done

if [ $# -gt 0 ] && [ "${1#v}" != "$cargo" ]; then
  echo "tag $1 does not match version $cargo" >&2
  status=1
fi

[ $status -eq 0 ] && echo "version $cargo"
exit $status
