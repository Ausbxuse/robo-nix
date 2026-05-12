# 2026-05-11 - Graphics Wrapper Simplification

## Goal

Reduce graphics-wrapper complexity now that nixGL owns non-NixOS host graphics
wrapping.

## Conflict Check

- `robo-nix` may select a host graphics wrapper, but it must not become one.
- `desktop-gl` is application/runtime client-library support, not a GPU driver
  selector.
- Let nixGL wrappers own graphics variables; robo imports their output without
  adding PRIME render-offload defaults.
- Keep runtime behavior truthful and explicit; do not add host scans or
  project-specific workarounds.

No active review-ledger conflict blocks this cleanup.

## Scope

- Give the generated Nix shell one source of truth for graphics variables
  imported from nixGL.
- Keep the `nixgl-nvidia` version detection path explicit and narrow.
- Replace deprecated Nixpkgs graphics package references.
- Keep the old explicit NVIDIA graphics spelling during this simplification
  pass.

## Non-Goals

- Do not remove host CUDA driver bridging.
- Do not add a new command surface.
- Do not change default `hostGraphics = "auto"` behavior.

## Verification

- [x] `cargo test`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
- [x] `nix flake check --impure`
- [x] docs build
