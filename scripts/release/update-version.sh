#!/usr/bin/env bash
set -euo pipefail

update_version_files() {
  local manifest="$1"
  local lockfile="$2"
  local version="$3"

  sed -i.bak -E "1,/^version = \".*\"$/s/^version = \".*\"$/version = \"$version\"/" "$manifest"
  rm -f "$manifest.bak"

  awk -v version="$version" '
    BEGIN { in_saferm = 0; updated = 0 }
    /^\[\[package\]\]$/ { in_saferm = 0 }
    /^name = "saferm"$/ { in_saferm = 1 }
    in_saferm && /^version = ".*"$/ && !updated {
      print "version = \"" version "\""
      updated = 1
      next
    }
    { print }
  ' "$lockfile" >"$lockfile.tmp"
  mv "$lockfile.tmp" "$lockfile"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  update_version_files "$1" "$2" "$3"
fi
