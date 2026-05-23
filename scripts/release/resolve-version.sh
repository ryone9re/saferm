#!/usr/bin/env bash
set -euo pipefail

semver_pattern='^[0-9]+\.[0-9]+\.[0-9]+$'

parse_semver() {
  local version="$1"
  local prefix="$2"

  [[ "$version" =~ $semver_pattern ]] || {
    echo "invalid $prefix version: $version" >&2
    return 1
  }

  IFS='.' read -r major minor patch <<<"$version"
  printf '%s %s %s\n' "$major" "$minor" "$patch"
}

resolve_version() {
  local requested="$1"
  local current="$2"
  local current_major current_minor current_patch

  read -r current_major current_minor current_patch < <(parse_semver "$current" "current")

  if [[ -n "$requested" ]]; then
    local requested_major requested_minor requested_patch
    read -r requested_major requested_minor requested_patch < <(parse_semver "$requested" "requested")

    if (( requested_major < current_major )) ||
      (( requested_major == current_major && requested_minor < current_minor )) ||
      (( requested_major == current_major && requested_minor == current_minor && requested_patch < current_patch )); then
      echo "requested version $requested is lower than current version $current" >&2
      return 1
    fi

    printf '%s\n' "$requested"
    return 0
  fi

  printf '%s.%s.%s\n' "$current_major" "$current_minor" "$((current_patch + 1))"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  resolve_version "${1:-}" "${2:-}"
fi
