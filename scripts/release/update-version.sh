#!/usr/bin/env bash
set -euo pipefail

update_version_files() {
  local manifest="$1"
  local lockfile="$2"
  local version="$3"
  local manifest_tmp="$manifest.tmp"
  local lockfile_tmp="$lockfile.tmp"

  awk -v version="$version" '
    BEGIN { in_package = 0; updated = 0 }
    /^\[package\]$/ {
      in_package = 1
      print
      next
    }
    /^\[.*\]$/ {
      in_package = 0
    }
    in_package && !updated && /^version = ".*"$/ {
      print "version = \"" version "\""
      updated = 1
      next
    }
    { print }
    END { if (!updated) exit 1 }
  ' "$manifest" >"$manifest_tmp" || {
    rm -f "$manifest_tmp" "$lockfile_tmp"
    echo "failed to update manifest version in $manifest" >&2
    return 1
  }

  if cmp -s "$manifest" "$manifest_tmp"; then
    rm -f "$manifest_tmp" "$lockfile_tmp"
    echo "manifest version unchanged in $manifest" >&2
    return 1
  fi

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
    END { if (!updated) exit 1 }
  ' "$lockfile" >"$lockfile_tmp" || {
    rm -f "$manifest_tmp" "$lockfile_tmp"
    echo "failed to update saferm package version in $lockfile" >&2
    return 1
  }

  if cmp -s "$lockfile" "$lockfile_tmp"; then
    rm -f "$manifest_tmp" "$lockfile_tmp"
    echo "saferm package version unchanged in $lockfile" >&2
    return 1
  fi

  mv "$manifest_tmp" "$manifest"
  mv "$lockfile_tmp" "$lockfile"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  update_version_files "$1" "$2" "$3"
fi
