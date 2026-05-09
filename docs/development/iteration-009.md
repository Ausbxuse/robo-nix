# Iteration 009 - Installer

## Goal

Add a minimal installer and document the install path without expanding the
current CLI surface.

## Scope

- Add a POSIX shell installer under `scripts/install.sh`.
- Expose a natural flake package target as `#robo`.
- Update user docs and README with install commands.
- Keep installer output aligned with the current `robo shell` workflow.

## Non-Goals

- No `robo init`.
- No `robo check`.
- No `robo diagnose`.
- No template publishing surface.
- No platform-specific package manager installers.

## Review Notes

Pending concerns:

- The default installer source points at `greenfield/minimal-core` while this
  branch is under review. Revisit before moving this product surface to a
  public release branch.
- The installer deliberately installs through Nix profiles. Other distribution
  channels should be separate reviewed iterations.
- Existing local installs may be named either `robo` or `robo-nix-minimal`,
  depending on whether they used `#robo` or the older `#default` target.
  Remove both before installing to avoid stale binaries or profile conflicts.

## Verification

Run for this iteration:

- `sh -n scripts/install.sh`
- `nix flake check --accept-flake-config`
- `npm --prefix docs run build`
- `cargo test`
