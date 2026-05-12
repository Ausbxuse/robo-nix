# 2026-05-09 - Explicit Host Graphics Policy

## Goal

Make host NVIDIA graphics selection normie-friendly without hiding host-driver
policy under generic runtime components.

## Conflict Check

- `desktop-gl` should continue to mean Nix-managed graphics and windowing
  libraries.
- Host CUDA driver bridging is still narrower than host graphics policy.
- The previous "no automatic Vulkan/EGL host bridge" rule remains valid for
  implicit behavior. This change adds an explicit manifest knob.

## Failure Observed

A downstream Isaac Sim smoke test had all generic Nix runtime libraries present
but still selected Mesa/Intel for Vulkan/EGL. Adding the NVIDIA Vulkan/EGL/GLX
environment variables made Isaac enumerate the NVIDIA GPU.

## Scope

- Add an explicit NVIDIA host graphics policy to `robo.nix`.
- Expand that policy into the host NVIDIA Vulkan/EGL/GLX
  environment variables.
- Include generated comments explaining the options.
- Warn Isaac Sim users when host CUDA is visible but no NVIDIA host graphics
  policy appears selected.

The 2026-05-10 NVIDIA manifest paths change later extends the explicit NVIDIA
policy from NixOS-only manifest paths to a reviewed NixOS/FHS distro candidate
list.

## Non-Goals

- No automatic host GPU scan.
- No mutation of existing `robo.nix`.
- No Isaac-specific component.

## Verification

- [x] `cargo test --no-default-features`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] render a temporary project and parse its generated `flake.nix` and
  `robo.nix`
- [x] parse generated `robo.nix` after setting explicit NVIDIA graphics policy

`cargo fmt --check` and `rustfmt --check` were not available on this host.
