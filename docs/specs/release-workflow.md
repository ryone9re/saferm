# Release Workflow Spec

## Summary

Replace the current tag-push-driven release process with a single manually triggered GitHub Actions workflow that:

- allows releases to be initiated independently from merges to `main`
- keeps the released version, `Cargo.toml`, `Cargo.lock`, CLI `--version`, and Git tag aligned
- verifies the release commit before anything is pushed to `main`
- requires explicit GitHub environment approval before publishing

The resulting operator flow is:

1. Open `Actions > Release`
2. Optionally enter a version
3. Run the workflow
4. Approve the `release` environment
5. Let the workflow push the release commit, tag it, and publish the GitHub Release

If no version is provided, the workflow increments the patch version from `Cargo.toml`.

## Context

The current process requires:

1. merge changes into `main`
2. prepare a separate version bump
3. merge the version bump
4. create and push a release tag

This is awkward because release timing is intentionally separate from merge timing, while `mise` distribution still depends on GitHub tags and releases.

## Goals

- Release from `main` only, on demand
- Require just one operator action plus approval
- Keep repository version metadata and released version identical
- Avoid pushing a version bump commit if release validation fails
- Preserve GitHub Releases and tags as the distribution source for `mise`
- Keep direct human pushes to `main` disallowed

## Non-Goals

- Reusing CI steps automatically from the existing CI workflow
- Releasing from arbitrary branches
- Supporting prerelease channels in the first version of this design
- Eliminating GitHub environment configuration work outside the repository

## Proposed Design

### 1. Replace release trigger model

Remove the current `push.tags: ["v*"]` release entrypoint and replace it with a dedicated `workflow_dispatch` workflow.

Inputs:

- `version` (optional): explicit semver such as `1.2.0`

Behavior:

- if `version` is empty, read the current version from `Cargo.toml` and increment the patch component
- derive the Git tag name as `v<version>`

### 2. Use `Cargo.toml` as the single source of truth

The workflow updates:

- `Cargo.toml`
- `Cargo.lock`

These files are updated before release validation so that:

- `saferm --version` matches the release version
- the release commit in `main` matches the published tag
- repository history reflects exactly what was released

### 3. Validate before publishing

The workflow creates a local release commit such as:

`chore: release v1.0.2`

This commit is not pushed immediately. Validation runs against that commit first.

Validation includes:

- format check
- lint
- test
- release build
- packaging of release artifacts

If any step fails, the workflow exits without pushing the commit or tag.

### 4. Require protected publishing

The final publish job uses `environment: release`.

GitHub-side requirements:

- configure a `release` environment
- require reviewer approval for that environment
- optionally disallow self-review

Publishing credentials:

- use a dedicated bot or GitHub App credential for the publish step
- allow only that actor to bypass `main` branch protection or rulesets

This ensures:

- normal development still requires PRs
- only the approved release workflow can add the release commit to `main`

### 5. Publish atomically from the validated commit

After approval, the workflow:

1. confirms `origin/main` has not moved since workflow preparation
2. pushes the validated release commit to `main`
3. tags that exact commit as `v<version>`
4. pushes the tag
5. creates the GitHub Release and uploads built artifacts

If `origin/main` changed before publish, the workflow fails rather than publishing from a stale base.

## Workflow Structure

### `prepare`

Responsibilities:

- check out `main`
- resolve the target version
- validate semver format when explicit input is provided
- fail if the tag already exists
- update `Cargo.toml` and `Cargo.lock`
- create the local release commit
- expose the resolved version, tag, base SHA, and release commit SHA as outputs

### `verify-build`

Responsibilities:

- run on the release commit created in `prepare`
- run verification commands
- build release artifacts for the supported targets
- archive artifacts for later publication

This job intentionally owns its own release validation set. It does not try to automatically mirror future CI workflow jobs.

### `publish`

Responsibilities:

- require `environment: release`
- re-check `origin/main`
- push the release commit to `main`
- push the tag
- create the GitHub Release from the same commit

## Version Rules

- explicit `version` input wins when provided
- empty `version` input triggers patch increment from the current `Cargo.toml` value
- only stable semver is supported initially
- tag names always use the `v` prefix

Examples:

- `1.0.1` with empty input becomes `1.0.2`
- explicit input `2.0.0` produces tag `v2.0.0`

## Failure Semantics

- invalid semver input: fail in `prepare`
- existing tag: fail in `prepare`
- version file update failure: fail in `prepare`
- validation failure: fail in `verify-build`, push nothing
- `main` moved before publish: fail in `publish`, push nothing
- release creation failure after commit/tag push: manual follow-up may be required, but the repository state remains consistent because the tag points at the committed release version

## Repository Changes

Planned repository changes:

- replace the existing release workflow with a manual release workflow
- add helper script(s) for version calculation and version file updates if shell-only YAML becomes too brittle
- update README release instructions to describe the manual workflow and approval gate

No change is planned to the CI workflow beyond whatever is needed to keep release validation readable and maintainable.

## Operational Notes

- operators should release only from the current `main`
- version bumps happen only through the release workflow
- regular feature PRs should not edit `Cargo.toml` version unless intentionally preparing a release-related change
- environment approval is the explicit release control point

## Testing Plan

Implementation should be validated with at least these cases:

1. no version input: patch bump is computed correctly
2. explicit version input: chosen version is applied consistently
3. duplicate tag: workflow fails before mutation
4. validation failure: no commit or tag is pushed
5. `main` advanced before publish: workflow aborts publish

## Open Questions Resolved

- Release timing remains independent from merge timing: yes
- `mise` distribution via GitHub tags/releases remains supported: yes
- `Cargo.toml` version and released version remain unified: yes
- CI and release validation are not forcibly unified: yes
