# 2026-05-11 - Conservative Auto Graphics

## Concern

On a non-NixOS Linux workstation, `hostGraphics = "nixgl-nvidia"` created an
NVIDIA GL context but a simulator's visible window stayed transparent. The
generic nixGL path rendered correctly, though slower. Selecting NVIDIA
automatically on any host with `nvidia-smi` therefore made the default less
robust.

## Decision

- Keep `hostGraphics = "nixgl-nvidia"` as an explicit opt-in for projects that
  require the NVIDIA nixGL wrapper.
- Make `hostGraphics = "auto"` use `/run/opengl-driver` on NixOS and the
  generic nixGL wrapper on other Linux hosts.
- Do not infer `nixgl-nvidia` merely from `nvidia-smi`.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
