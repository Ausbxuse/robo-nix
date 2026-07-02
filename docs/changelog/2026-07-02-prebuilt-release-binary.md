# Prebuilt release binary

## Context

The published tags currently stop at `v0.1.0-beta.2`, while the package version
is `0.1.1`. There was no GitHub Actions release workflow, so tagging the current
version would not attach a prebuilt `robo` binary to the GitHub Release.

No active review-ledger conflict blocks adding release packaging at the GitHub
Release boundary.

## Change

- Add a tag-triggered release workflow for `v*` tags.
- Validate that the release tag matches the Cargo package version before
  publishing artifacts.
- Build the Linux x86_64 `robo` binary with locked Cargo dependencies.
- Attach a `tar.gz` package and SHA-256 checksum to the GitHub Release.

## Verification

- `nix-instantiate --parse flake.nix`
- `cargo metadata --locked --no-deps --format-version 1`
- `cargo test`
- `cargo build --locked --release`
- Parsed `.github/workflows/release.yml` with PyYAML.
- Locally packaged `target/release/robo` into the release tarball shape and
  verified the archive contains `robo` and `LICENSE`.
