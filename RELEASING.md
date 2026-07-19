# Releasing OpenTFRaw

This repo publishes two packages from a single version number: the
`opentfraw` crate to crates.io, and the `opentfraw` Python package (built
from `crates/opentfraw-py` via maturin) to PyPI. Both are published by
`.github/workflows/publish.yml`, which triggers on pushing a `v*` tag.

`publish.yml` has no dependency on `.github/workflows/ci.yml` or
`.github/workflows/audit.yml` passing - GitHub Actions can't make one
workflow file `needs:` a job in another workflow file. That means CI/audit
status has to be checked *before* tagging, which is what
`scripts/check-release-ready.sh` is for. Don't skip it.

## Steps

1. **Confirm `main` is ready.** Make sure everything you want in the
   release is merged to `main` and `git log` / `CHANGELOG.md`'s
   `[Unreleased]` section reflect it.

2. **Check CI and audit are green for the target commit:**

   ```sh
   ./scripts/check-release-ready.sh
   ```

   This checks the latest `ci.yml` and `audit.yml` runs for `HEAD` (pass a
   different ref/SHA to check something other than `HEAD`). It exits
   non-zero and prints the run URL if either workflow hasn't run for that
   commit, is still in progress, or didn't succeed. Do not proceed to
   tagging until this passes.

3. **Bump the version.** There is one version number for the whole
   workspace, in the root `Cargo.toml`:

   ```toml
   [workspace.package]
   version = "X.Y.Z"
   ```

   Both `crates/opentfraw/Cargo.toml` and `crates/opentfraw-py/Cargo.toml`
   inherit it via `version.workspace = true`, and the Python package's
   version (`pyproject.toml` declares it `dynamic`) is read from the Rust
   crate at build time via maturin, so this is the only place to edit.
   Run `cargo build` (or any cargo command) afterwards to update
   `Cargo.lock`.

4. **Update `CHANGELOG.md`.** Retitle the `## [Unreleased]` section to
   `## [X.Y.Z] - YYYY-MM-DD` (today's date) and start a fresh empty
   `## [Unreleased]` section above it. Follow the existing
   [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) style already
   used in this file.

5. **Commit.** Follow the existing convention for release commits, e.g.:

   ```
   release: vX.Y.Z
   ```

   (see `git log --grep=^release:` for prior examples).

6. **Push to `main` and let CI/audit run on the release commit itself,**
   then re-run `./scripts/check-release-ready.sh` against that new commit
   (it defaults to `HEAD`, so this is normally just running it again once
   CI/audit have finished). Don't tag until it passes for the exact
   commit you're about to tag.

7. **Tag and push the tag:**

   ```sh
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

   Pushing the tag triggers `publish.yml`, which publishes to crates.io
   (`cargo-publish` job) and builds + publishes wheels/sdist to PyPI
   (`build-wheels` / `build-sdist` / `pypi-publish` jobs).

8. **Verify the publish.** Check the
   [Publish workflow run](https://github.com/Sigilweaver/OpenTFRaw/actions/workflows/publish.yml)
   succeeded, then confirm the new version shows up on
   [crates.io](https://crates.io/crates/opentfraw) and
   [PyPI](https://pypi.org/project/opentfraw/).
