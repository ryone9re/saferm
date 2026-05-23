# Release Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace tag-push releases with a manual GitHub Actions release flow that computes the target version, validates a release candidate before publish, and only then pushes the release commit and tag to `main`.

**Architecture:** Keep the release policy in `.github/workflows/release.yaml`, but move version resolution and manifest updates into small shell helpers under `scripts/release/` so the workflow logic stays readable. Preserve the exact validated release candidate across jobs by bundling the local git commit from the preparation step, then publish that same commit after environment approval.

**Tech Stack:** GitHub Actions, POSIX shell, git bundle, Cargo, README documentation

---

### Task 1: Add release helper scripts with shell tests

**Files:**
- Create: `scripts/release/resolve-version.sh`
- Create: `scripts/release/update-version.sh`
- Create: `scripts/tests/release_helpers_test.sh`

- [ ] **Step 1: Write the failing shell tests**

Create `scripts/tests/release_helpers_test.sh`:

```bash
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
  trap 'rm -rf "$tmpdir"' RETURN

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bash scripts/tests/release_helpers_test.sh`
Expected: FAIL with `No such file or directory` while sourcing `scripts/release/resolve-version.sh`

- [ ] **Step 3: Write the minimal helper implementations**

Create `scripts/release/resolve-version.sh`:

```bash
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
```

Create `scripts/release/update-version.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

update_version_files() {
  local manifest="$1"
  local lockfile="$2"
  local version="$3"

  sed -i.bak -E "0,/^version = \".*\"$/s//version = \"$version\"/" "$manifest"
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bash scripts/tests/release_helpers_test.sh`
Expected: PASS with `release helper tests: ok`

- [ ] **Step 5: Commit**

```bash
git add scripts/release/resolve-version.sh scripts/release/update-version.sh scripts/tests/release_helpers_test.sh
git commit -m "chore: add release helper scripts"
```

### Task 2: Replace the release workflow with a manual publish flow

**Files:**
- Modify: `.github/workflows/release.yaml`
- Test: `scripts/tests/release_helpers_test.sh`

- [ ] **Step 1: Write the failing workflow assertions into the helper tests**

Append this test to `scripts/tests/release_helpers_test.sh`:

```bash
test_resolve_version_rejects_prerelease_suffix() {
  if resolve_version "1.0.2-rc1" "1.0.1" >/dev/null 2>&1; then
    fail "prerelease versions should fail in the first implementation"
  fi
}
```

and call it from `main()`:

```bash
  test_resolve_version_rejects_prerelease_suffix
```

- [ ] **Step 2: Run test to verify it fails for the new rule**

Run: `bash scripts/tests/release_helpers_test.sh`
Expected: FAIL with `prerelease versions should fail in the first implementation`

- [ ] **Step 3: Update the helper and replace `.github/workflows/release.yaml`**

Keep `scripts/release/resolve-version.sh` semver validation strict, then replace `.github/workflows/release.yaml` with:

```yaml
name: Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: "Release version (optional; defaults to patch bump)"
        required: false
        type: string

env:
  CARGO_TERM_COLOR: always

jobs:
  prepare:
    name: Prepare release candidate
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.meta.outputs.version }}
      tag: ${{ steps.meta.outputs.tag }}
      base_sha: ${{ steps.meta.outputs.base_sha }}
    steps:
      - name: Checkout main
        uses: actions/checkout@v6
        with:
          ref: main
          fetch-depth: 0

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Resolve version
        id: meta
        shell: bash
        run: |
          set -euo pipefail
          current_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
          version="$(bash scripts/release/resolve-version.sh "${{ inputs.version }}" "$current_version")"
          tag="v$version"
          git fetch --tags origin
          if git rev-parse "$tag" >/dev/null 2>&1; then
            echo "tag already exists: $tag" >&2
            exit 1
          fi
          echo "version=$version" >>"$GITHUB_OUTPUT"
          echo "tag=$tag" >>"$GITHUB_OUTPUT"
          echo "base_sha=$(git rev-parse HEAD)" >>"$GITHUB_OUTPUT"

      - name: Update version files
        run: bash scripts/release/update-version.sh Cargo.toml Cargo.lock "${{ steps.meta.outputs.version }}"

      - name: Create release commit
        shell: bash
        run: |
          set -euo pipefail
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add Cargo.toml Cargo.lock
          git commit -m "chore: release ${{ steps.meta.outputs.tag }}"

      - name: Bundle release candidate
        shell: bash
        run: |
          set -euo pipefail
          git branch -f release-candidate HEAD
          git bundle create release.bundle release-candidate ^"${{ steps.meta.outputs.base_sha }}"
          git rev-parse HEAD >release_commit_sha.txt

      - name: Upload release candidate
        uses: actions/upload-artifact@v6
        with:
          name: release-candidate
          path: |
            release.bundle
            release_commit_sha.txt

  verify-build:
    name: Verify and build release candidate
    needs: prepare
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-musl
            runner: ubuntu-latest
            use_cross: true
          - target: aarch64-apple-darwin
            runner: macos-latest
            use_cross: false
    steps:
      - name: Checkout main
        uses: actions/checkout@v6
        with:
          ref: main
          fetch-depth: 0

      - name: Download release candidate
        uses: actions/download-artifact@v7
        with:
          name: release-candidate

      - name: Import release candidate commit
        shell: bash
        run: |
          set -euo pipefail
          git fetch ./release.bundle 'refs/heads/release-candidate:refs/remotes/bundle/release-candidate'
          git checkout "$(cat release_commit_sha.txt)"

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
          targets: ${{ matrix.target }}

      - name: Cache
        uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}-release

      - name: Format check
        run: cargo fmt -- --check

      - name: Lint
        run: cargo clippy -- -D warnings

      - name: Test
        run: cargo test

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build (cross)
        if: matrix.use_cross
        run: cross build --release --target ${{ matrix.target }}

      - name: Build (cargo)
        if: ${{ !matrix.use_cross }}
        run: cargo build --release --target ${{ matrix.target }}

      - name: Archive
        shell: bash
        run: |
          set -euo pipefail
          cd "target/${{ matrix.target }}/release"
          tar czf "../../../saferm-${{ matrix.target }}.tar.gz" saferm

      - name: Upload build artifact
        uses: actions/upload-artifact@v6
        with:
          name: saferm-${{ matrix.target }}
          path: saferm-${{ matrix.target }}.tar.gz

  publish:
    name: Publish release
    needs: [prepare, verify-build]
    runs-on: ubuntu-latest
    environment: release
    permissions:
      contents: write
    steps:
      - name: Checkout main
        uses: actions/checkout@v6
        with:
          ref: main
          fetch-depth: 0
          token: ${{ secrets.RELEASE_PUSH_TOKEN }}

      - name: Download release candidate
        uses: actions/download-artifact@v7
        with:
          name: release-candidate

      - name: Download packaged artifacts
        uses: actions/download-artifact@v7
        with:
          pattern: saferm-*
          merge-multiple: true

      - name: Verify main head and import release commit
        shell: bash
        run: |
          set -euo pipefail
          git fetch origin main --tags
          current_main="$(git rev-parse origin/main)"
          if [[ "$current_main" != "${{ needs.prepare.outputs.base_sha }}" ]]; then
            echo "origin/main advanced from ${{ needs.prepare.outputs.base_sha }} to $current_main" >&2
            exit 1
          fi
          git fetch ./release.bundle 'refs/heads/release-candidate:refs/remotes/bundle/release-candidate'
          git checkout "$(cat release_commit_sha.txt)"

      - name: Push release commit and tag
        shell: bash
        run: |
          set -euo pipefail
          git push origin HEAD:main
          git tag "${{ needs.prepare.outputs.tag }}"
          git push origin "${{ needs.prepare.outputs.tag }}"

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.prepare.outputs.tag }}
          generate_release_notes: true
          files: saferm-*.tar.gz
```

- [ ] **Step 4: Run the local verification and helper tests**

Run: `bash scripts/tests/release_helpers_test.sh && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Expected: PASS, and `scripts/tests/release_helpers_test.sh` prints `release helper tests: ok`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yaml scripts/release/resolve-version.sh scripts/release/update-version.sh scripts/tests/release_helpers_test.sh
git commit -m "feat: add manual release workflow"
```

### Task 3: Update documentation for the new release flow

**Files:**
- Modify: `README.md`
- Test: `.github/workflows/release.yaml`

- [ ] **Step 1: Write the failing documentation expectation**

In your notes, define the expected release section content:

```text
The README must no longer tell the operator to create and push a tag manually.
It must say that releases are started from Actions, that version input is optional,
that an empty version performs a patch bump, and that release publishing waits for
the GitHub `release` environment approval.
```

- [ ] **Step 2: Verify the current README still documents the old flow**

Run: `rg -n "git tag|git push origin v|Actions > Release|patch bump|environment" README.md`
Expected: matches only the old `git tag v1.2.0` / `git push origin v1.2.0` instructions, and no `Actions > Release` guidance

- [ ] **Step 3: Replace the README release instructions**

Update the release section in `README.md` to:

```md
When you want to publish a release, run the `Release` workflow from GitHub Actions.
The workflow always releases from `main`, updates `Cargo.toml` and `Cargo.lock`,
validates the release candidate, and waits for approval from the `release`
environment before publishing.

- Leave the version input empty to bump the patch version automatically
- Provide a version such as `1.2.0` to override the automatic bump

After approval, the workflow pushes the release commit to `main`, creates the
matching `v*` tag, builds release artifacts for the supported targets, and
publishes the GitHub Release.

```bash
# Release from GitHub Actions
Actions > Release > Run workflow
```
```

- [ ] **Step 4: Run the documentation and code verification**

Run: `rg -n "Actions > Release|release environment" README.md && ! rg -n "git tag v[0-9]" README.md && bash scripts/tests/release_helpers_test.sh && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Expected: README references `Actions > Release` and the release environment, does not keep the old manual tag commands, and all checks pass

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: document manual release workflow"
```

### Task 4: Final pre-merge verification and operator handoff

**Files:**
- Modify: none
- Test: `.github/workflows/release.yaml`

- [ ] **Step 1: Re-run the full implementation verification**

Run:

```bash
bash scripts/tests/release_helpers_test.sh
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

Expected: all commands succeed with no modified files left behind

- [ ] **Step 2: Review the final diff for the intended surface area**

Run:

```bash
git diff --stat main...
git diff main... -- .github/workflows/release.yaml README.md scripts/release scripts/tests
```

Expected: only the new manual release flow, helper scripts, tests, and README changes appear

- [ ] **Step 3: Document the GitHub-side setup that cannot be committed**

Add this handoff note to the PR description or implementation summary:

```text
Post-merge GitHub configuration:
- create a `release` environment
- require reviewer approval for the environment
- optionally disable self-review
- store the publish credential as `RELEASE_PUSH_TOKEN`
- allow only that bot or GitHub App to bypass `main` protection for release pushes
```

- [ ] **Step 4: Commit any final touch-ups**

```bash
git status --short
```

Expected: no output. If there is output, stage only the intended follow-up fixes and commit with a focused message.
