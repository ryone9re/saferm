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

test_same_version_is_allowed_for_recovery() {
  local version
  version="$(resolve_version "1.0.1" "1.0.1")"
  assert_eq "$version" "1.0.1" "same explicit version is allowed for recovery"
}

test_explicit_version_downgrade_fails() {
  if resolve_version "1.0.0" "1.0.1" >/dev/null 2>&1; then
    fail "explicit version downgrade should fail"
  fi
}

test_invalid_version_fails() {
  if resolve_version "1.0" "1.0.1" >/dev/null 2>&1; then
    fail "invalid semver should fail"
  fi
}

test_resolve_version_rejects_prerelease_suffix() {
  if resolve_version "1.0.2-rc1" "1.0.1" >/dev/null 2>&1; then
    fail "prerelease versions should fail in the first implementation"
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

test_update_version_fails_when_manifest_version_missing() {
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap "rm -rf '$tmpdir'" RETURN

  cat >"$tmpdir/Cargo.toml" <<'EOF'
[package]
name = "saferm"
edition = "2024"
EOF

  cat >"$tmpdir/Cargo.lock" <<'EOF'
version = 4

[[package]]
name = "saferm"
version = "1.0.1"
EOF

  if update_version_files "$tmpdir/Cargo.toml" "$tmpdir/Cargo.lock" "1.0.2" >/dev/null 2>&1; then
    fail "missing manifest version should fail"
  fi
}

test_update_version_fails_when_package_version_missing_but_other_table_has_version() {
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap "rm -rf '$tmpdir'" RETURN

  cat >"$tmpdir/Cargo.toml" <<'EOF'
[package]
name = "saferm"
edition = "2024"

[metadata.release]
version = "9.9.9"
EOF

  cat >"$tmpdir/Cargo.lock" <<'EOF'
version = 4

[[package]]
name = "saferm"
version = "1.0.1"
EOF

  if update_version_files "$tmpdir/Cargo.toml" "$tmpdir/Cargo.lock" "1.0.2" >/dev/null 2>&1; then
    fail "missing package version should fail even when another table has version"
  fi
}

test_update_version_fails_when_lockfile_saferm_package_missing() {
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
name = "another-package"
version = "1.0.1"
EOF

  if update_version_files "$tmpdir/Cargo.toml" "$tmpdir/Cargo.lock" "1.0.2" >/dev/null 2>&1; then
    fail "missing saferm package in lockfile should fail"
  fi
}

main() {
  test_patch_bump_when_input_empty
  test_explicit_version_wins
  test_same_version_is_allowed_for_recovery
  test_explicit_version_downgrade_fails
  test_invalid_version_fails
  test_resolve_version_rejects_prerelease_suffix
  test_update_version_rewrites_manifest_and_lock
  test_update_version_fails_when_manifest_version_missing
  test_update_version_fails_when_package_version_missing_but_other_table_has_version
  test_update_version_fails_when_lockfile_saferm_package_missing
  echo "release helper tests: ok"
}

main "$@"
