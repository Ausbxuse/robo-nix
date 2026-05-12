# Iteration 040 - nixGL Graphics Boundary

## Goal

Keep host graphics policy small now that nixGL owns the non-NixOS OpenGL
wrapper path.

## Conflict Check

- Keep Nix-managed desktop graphics separate from host NVIDIA driver policy.
- Do not maintain Rust-owned GLX/EGL/GBM graphics wrapping.
- `hostGraphics = "nvidia"` remains a compatibility alias for
  `hostGraphics = "nixgl-nvidia"`.
- Runtime shell setup should be standalone and should not require users to
  install nixGL wrappers into their profile.

No active review-ledger conflict blocks this cleanup.

## Failure Observed

On a non-NixOS Linux workstation, generic nixGL rendered correctly but slowly.
The NVIDIA nixGL path created a hardware GL context while the visible simulator
window could stay transparent. The host session was an Xorg desktop with NVIDIA
as the primary output provider, so forcing PRIME render-offload variables on
top of the nixGLNvidia wrapper was outside robo-nix's ownership boundary and
could push presentation through the wrong path.

## Scope

- Let nixGL wrappers provide graphics variables without adding robo-owned PRIME
  render-offload defaults.
- Remove profile `PATH` probing for nixGL wrappers; use bundled nixGL or the
  explicit `ROBO_NIX_NIXGL` override.
- Generalize development notes that mentioned downstream projects or host names.

## Non-Goals

- Do not add a new diagnose command.
- Do not scan host graphics driver directories from Rust.
- Do not remove the compatibility `hostGraphics = "nvidia"` spelling in this
  pass.

## Verification

- [x] `cargo test`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix flake check --impure`
- [x] generated project smoke test
