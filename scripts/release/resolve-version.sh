#!/usr/bin/env bash
set -euo pipefail

semver_pattern='^[0-9]+\.[0-9]+\.[0-9]+$'

resolve_version() {
  local requested="$1"
  local current="$2"

  if [[ -n "$requested" ]]; then
    [[ "$requested" =~ $semver_pattern ]] || {
      echo "invalid semver: $requested" >&2
      return 1
    }
    printf '%s\n' "$requested"
    return 0
  fi

  [[ "$current" =~ $semver_pattern ]] || {
    echo "invalid current version: $current" >&2
    return 1
  }

  IFS='.' read -r major minor patch <<<"$current"
  printf '%s.%s.%s\n' "$major" "$minor" "$((patch + 1))"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  resolve_version "${1:-}" "${2:-}"
fi
