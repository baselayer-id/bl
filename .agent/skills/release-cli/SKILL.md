---
name: release-cli
description: How to cut a new release of the `bl` CLI. Covers version bumping, the `v<semver>` tag convention, the push → CI → Homebrew tap flow, and verification steps. Use this skill when asked to release, cut, tag, ship, publish, or bump the CLI — or when an existing release failed and needs to be debugged or re-run.
---

# Cutting a `bl` CLI release

A release is a single `git tag v<semver>` push. Everything else is CI —
`.github/workflows/release.yml` builds, packages, creates the GitHub Release,
and fires a `repository_dispatch` to the Homebrew tap.

## Happy path

```bash
cd ~/src/baselayer-id/bl

# 1. Bump Cargo.toml version (must match what the tag will be)
#    Edit the `version = "..."` line in Cargo.toml

# 2. Commit the bump
git add Cargo.toml
git commit -m "chore: bump to v<version>"

# 3. Tag with a matching `v` prefix and push both
git tag v<version>
git push origin main
git push origin v<version>
```

Pushing the tag triggers the release workflow. That's it — no manual artifact
uploads, no manual formula edits.

## Tag format

- **Prefix is required**: `v<semver>` — the workflow trigger is `tags: 'v*'`
- **Release candidates**: `v0.1.0-rc4`, `v0.1.0-rc5` — pre-release semver is fine
- **Stable**: `v0.1.0`, `v0.1.1`, `v0.2.0`
- **Never reuse** a tag. If a release fails mid-way, cut a new patch tag
  rather than re-pushing (force-pushing tags breaks Homebrew caching)

Check the latest tag before picking a new one:

```bash
git tag -l --sort=-v:refname | head -5
```

Current latest: `v0.1.0-rc4` (as of last check).

## Cargo.toml version

CI stamps `Cargo.toml` with the tag value at build time (see the "Stamp
Cargo.toml with tag version" step in `release.yml`), so the published binary
reports the tag version even if the committed `Cargo.toml` lags behind.

**Still bump it.** Reasons:

- Local `cargo build` reports the right version during development
- There's a CI version-drift check that flags unreleased local mismatches
- Diff hygiene — the bump commit makes the release discoverable in `git log`

The bump commit can piggy-back on other changes (e.g., `chore: bump to v0.1.1
+ add gemini hook`), or be its own commit — both are fine.

## What CI does on tag push

From `.github/workflows/release.yml`:

1. **`build` job** (matrix: `aarch64-apple-darwin`, `x86_64-apple-darwin`)
   - Stamps Cargo.toml with tag version (BSD sed — macOS runner)
   - `cargo build --release --target <arch>`
   - Packages `bl-<arch>.tar.gz` + `.sha256`
   - Uploads as artifact
2. **`universal` job**
   - Downloads both per-arch builds
   - `lipo`-merges into a universal binary
   - Produces three tarballs (arm64, x86_64, universal) + a consolidated `checksums.txt`
   - Creates the GitHub Release via `softprops/action-gh-release@v2`
3. **`update-tap` job**
   - Fires `repository_dispatch` to `baselayer-id/homebrew-tap`
   - Event: `update-formula`
   - Payload: `{"version": "<tag-minus-v>"}`
   - Uses the `TAP_REPO_TOKEN` secret (fine-grained PAT with `contents: write`
     on the tap repo)

The tap's `update-formula.yml` downloads the new release artifacts, computes
SHA256s, rewrites `Formula/bl.rb`, commits and pushes — so `brew upgrade bl`
works for users within a few minutes of the push.

## Verification checklist

After pushing the tag, verify the pipeline end-to-end:

```bash
# 1. Workflow triggered and is running / finished
gh run list --repo baselayer-id/bl --limit 3

# 2. Release is published with all artifacts
gh release view v<version> --repo baselayer-id/bl

# Expected assets:
#   bl-aarch64-apple-darwin.tar.gz
#   bl-x86_64-apple-darwin.tar.gz
#   bl-universal-apple-darwin.tar.gz
#   checksums.txt

# 3. Homebrew tap formula was updated
gh api repos/baselayer-id/homebrew-tap/commits --jq '.[0].commit.message' | head -1
# Should show a recent auto-commit bumping to <version>

# 4. End-to-end install test (fresh prefix)
brew update && brew upgrade baselayer-id/tap/bl
bl --version  # should match <version>
```

## Common failures

### Workflow didn't trigger

Most likely the tag doesn't start with `v`. Check:

```bash
git tag -l --points-at HEAD
```

If the tag is named wrong, delete locally and remotely, then re-tag:

```bash
git tag -d <wrong-tag>
git push origin :refs/tags/<wrong-tag>
git tag v<version>
git push origin v<version>
```

### `update-tap` job failed with 403/404

The `TAP_REPO_TOKEN` secret is missing, expired, or lacks `contents: write`
on `baselayer-id/homebrew-tap`. Check:

```bash
gh secret list --repo baselayer-id/bl
```

Regenerate the fine-grained PAT at https://github.com/settings/tokens?type=beta
(scoped only to the tap repo), update the secret:

```bash
gh secret set TAP_REPO_TOKEN --repo baselayer-id/bl
```

Then re-run the failed job (`gh run rerun <run-id> --repo baselayer-id/bl
--failed`) — no need to cut a new tag.

### `lipo: can't open input file`

The arch-specific build failed upstream. Check the matrix `build` job logs —
usually a Rust toolchain issue or a missing dep, not a lipo problem.

### Release created but formula didn't update

The tap dispatch fired but the tap workflow failed. Check:

```bash
gh run list --repo baselayer-id/homebrew-tap --limit 3
```

You can manually re-trigger:

```bash
gh api repos/baselayer-id/homebrew-tap/dispatches \
  -f event_type=update-formula \
  -f 'client_payload[version]=<version>'
```

Or fall back to the manual path in the CLI repo:

```bash
bash homebrew/update-formula.sh <version>
# Then hand-copy homebrew/bl.rb to the tap repo
```

### Published wrong version / need to retract

Don't delete the release. Cut a new patch:

- If version was `v0.1.0` and it's broken → tag `v0.1.1` with the fix
- If it's an rc (`v0.1.0-rc4`) → just tag `v0.1.0-rc5`
- Yanking from Homebrew requires a manual PR to the tap revering the formula
  — message the user first, don't do this autonomously

## Manual workflow dispatch (re-run without a new tag)

The workflow accepts a `workflow_dispatch` input for the version. Use this
only when CI failed on a good tag and you want to retry the build without
creating a new release:

```bash
gh workflow run release.yml --repo baselayer-id/bl -f version=0.1.0-rc4
```

Note: this **overwrites** the release artifacts for that tag. Only safe if
the previous run failed before publishing — if there's already a working
release, cut a new tag instead.

## What not to do

- **Don't force-push tags.** `git push --force origin v<tag>` will rewrite
  the release but Homebrew caches by SHA — the tap workflow may refuse to
  update, or users who already downloaded will get checksum mismatches.
- **Don't commit a Cargo.toml version change without a matching tag.** The
  version-drift check will flag it on the next PR.
- **Don't edit Formula/bl.rb in the tap repo by hand** unless the auto-update
  path is actually broken — the tap workflow is the source of truth.
- **Don't cut a release from a branch other than `main`.** The tag should
  point at a commit that exists on main.
