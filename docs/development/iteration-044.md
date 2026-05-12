# Iteration 044 - Runtime Shell Contract Cleanup

## Goal

Make runtime shell preparation easier to audit after moving host graphics
wrapping to nixGL.

## Conflict Check

- Keep host graphics policy explicit and let nixGL own non-NixOS graphics
  wrapping.
- Keep `robo shell` and `robo run` on the same runtime preparation path.
- Do not add a new command surface or restore deleted compatibility aliases.
- Keep generated shell behavior transparent rather than silently patching
  project declarations.

No active review-ledger conflict blocks this cleanup.

## Scope

- Replace repeated generated-shell path-prepend blocks with one local helper.
- Keep the managed refresh environment contract in one auditable Rust list.
- Preserve existing runtime behavior for graphics, CUDA, Linux headers, and
  virtualenv path setup.

## Non-Goals

- Do not change default `hostGraphics = "auto"` behavior.
- Do not remove host CUDA driver bridging.
- Do not add host graphics scans or project-specific branches.

## Verification

- [x] `cargo test`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated `hostGraphics = "nixgl-nvidia"` smoke test
- [x] generated project rejects `hostGraphics = "nvidia"`
- [x] `nix flake check --impure`
- [x] docs build
