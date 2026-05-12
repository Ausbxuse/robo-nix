# Iteration 041 - Release Graphics Philosophy

## Goal

Make the release-facing docs and code language match the nixGL boundary.

## Conflict Check

- `robo-nix` should stay a focused runtime environment tool, not a general
  environment manager.
- Host graphics wrapping belongs to `/run/opengl-driver` on NixOS and nixGL on
  non-NixOS Linux.
- `desktop-gl` is application/runtime client-library support, not a host driver
  selector.
- The old explicit NVIDIA graphics spelling was still an alias for
  `hostGraphics = "nixgl-nvidia"` in this pass.

No active review-ledger conflict blocks this release cleanup.

## Decision

The durable product rule is:

> `robo-nix` may select a host graphics wrapper, but it must not become one.

Release docs should describe wrapper selection and imported graphics variables,
not a robo-owned host graphics wrapper implementation. The historical development
ledger can still describe older iterations, but current docs and code should
point users at the current boundary.

## Scope

- Update README, runtime docs, troubleshooting, and developer overview to use
  host graphics wrapper/import language.
- Make `desktop-gl` docs explicit that it provides client libraries and does
  not select the GPU driver.
- Keep graphics environment variables as nixGL-imported shell state.
- Rename the Isaac warning helper around graphics wrapper selection.
- Keep current behavior unchanged.

## Non-Goals

- Do not delete historical iteration notes.
- Do not remove the old explicit NVIDIA graphics spelling.
- Do not add a new diagnose command.

## Verification

- [x] `cargo test`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
- [x] `nix flake check --impure`
- [x] docs build
