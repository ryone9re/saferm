#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$ROOT/scripts/release/resolve-version.sh"
source "$ROOT/scripts/release/update-version.sh"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_eq() {
  local actual="$1"
  local expected="$2"
  local label="$3"

  if [[ "$actual" != "$expected" ]]; then
    fail "$label: expected '$expected', got '$actual'"
  fi
}

test_patch_bump_when_input_empty() {
  local version
  version="$(resolve_version "" "1.0.1")"
  assert_eq "$version" "1.0.2" "empty input bumps patch"
}

test_explicit_version_wins() {
  local version
  version="$(resolve_version "2.0.0" "1.0.1")"
  assert_eq "$version" "2.0.0" "explicit version is preserved"
}

test_invalid_version_fails() {
  if resolve_version "1.0" "1.0.1" >/dev/null 2>&1; then
    fail "invalid semver should fail"
  fi
}

test_update_version_rewrites_manifest_and_lock() {
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap "rm -rf '$tmpdir'" RETURN

  cat >"$tmpdir/Cargo.toml" <<'EOF'
[package]
name = "saferm"
version = "1.0.1"
edition = "2024"
EOF

  cat >"$tmpdir/Cargo.lock" <<'EOF'
version = 4

[[package]]
name = "saferm"
version = "1.0.1"
EOF

  update_version_files "$tmpdir/Cargo.toml" "$tmpdir/Cargo.lock" "1.0.2"

  assert_eq "$(grep '^version = ' "$tmpdir/Cargo.toml" | head -1)" 'version = "1.0.2"' "manifest version updated"
  assert_eq "$(grep '^version = ' "$tmpdir/Cargo.lock" | tail -1)" 'version = "1.0.2"' "lockfile root package version updated"
}

main() {
  test_patch_bump_when_input_empty
  test_explicit_version_wins
  test_invalid_version_fails
  test_update_version_rewrites_manifest_and_lock
  echo "release helper tests: ok"
}

main "$@"
