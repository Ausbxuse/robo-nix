# 2026-05-12 - Beta Readiness Refactor

## Goal

Prepare the next beta tag by making the implementation boundaries easier to
review and maintain while preserving the current product surface.

## Conflict Check

- Keep `robo shell`, `robo run`, and `robo search` as the only public command
  surface.
- Keep uv as the Python dependency owner; do not create `pyproject.toml`, run
  `uv sync`, or resolve Python package metadata remotely.
- Keep `robo.nix` canonical after first creation; do not rewrite it during
  shell preparation or refresh.
- Keep host CUDA driver bridging in Rust and host graphics policy in the Nix
  runtime shell.
- Keep nixGL as the owner of non-NixOS graphics wrapper variables.
- Keep generated project files and hidden implementation templates under `src/`.

No review-ledger conflict blocks a boundary-preserving refactor.

## Scope

- Rename the internal numbered development ledger to a date-based changelog.
- Split large Rust runtime modules by ownership boundary without changing CLI
  behavior.
- Move bulky Nix shell helper scripts out of the central project flake into
  shipped template files embedded from `src/`.
- Update developer guidance and docs build exclusion paths for the renamed
  changelog ledger.

## Non-Goals

- No new commands, components, or runtime inference rules.
- No host graphics behavior change.
- No CUDA driver probe expansion.
- No release tagging in this change.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `npm --prefix docs run build`
- [x] `nix build --accept-flake-config .#robo`
- [x] `nix fmt -- --check flake.nix src/nix/project-flake.nix`
- [x] `nix flake check --accept-flake-config`
