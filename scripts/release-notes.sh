#!/bin/sh
# Prints the CHANGELOG.md section for a version, for the release notes.
#
#   scripts/release-notes.sh 2.0.0
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
version=${1#v}
notes=$(awk -v v="$version" '
  index($0, "## [" v "]") == 1 { found = 1; next }
  found && /^## \[/ { exit }
  found { print }
' "$root/CHANGELOG.md")

if [ -z "$(printf '%s' "$notes" | tr -d '[:space:]')" ]; then
  echo "CHANGELOG.md has no section for $version" >&2
  exit 1
fi
printf '%s\n' "$notes"
