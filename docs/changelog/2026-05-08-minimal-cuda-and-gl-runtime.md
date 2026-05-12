# 2026-05-08 - Minimal CUDA And GL Runtime

## Goal

Add enough CUDA and desktop graphics runtime support that first-bootstrap
projects can express common robot-learning native runtime needs without bringing
back broad diagnostics or host-specific path guessing.

## Scope

- Add a `cuda-toolkit` component for Nix-owned CUDA native build surface:
  `nvcc`, headers, CCCL headers, `libcudart`, and common CUDA compile/link
  environment variables.
- Expand `desktop-gl` from a tiny OpenGL set to a broader Nix-managed desktop
  graphics runtime with EGL, GLVND, Vulkan loader, X11, Wayland, font, and DBus
  libraries.
- Keep host NVIDIA driver ownership explicit. `robo shell` must not scan host
  NVIDIA directories. It may honor `ROBO_NIX_LIBCUDA_PATH` when the user provides
  a driver library or directory.
- Extend first-bootstrap runtime inference for packages that clearly imply CUDA
  native build support or desktop graphics runtime support.
- Keep existing `robo.nix` canonical. These changes affect new first bootstrap
  and manual user edits only.

## Non-Goals

- No `robo check`.
- No `robo diagnose`.
- No host EGL/Vulkan/NVIDIA manifest bridge.
- No automatic CUDA driver discovery.
- No CUDA wheel/driver compatibility solver.

## Review Notes

Pending concerns:

- None yet.

## Verification

Run for this change:

- `cargo check`
- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Smoke bootstrap in `/tmp/robo-iter3-smoke` using dependencies
  `cuda-python`, `cupy-cuda12x`, and `mujoco`.
- `nix-instantiate --parse flake.nix` in the smoke project.
- `nix-instantiate --parse robo.nix` in the smoke project.
- `nix eval --accept-flake-config --no-write-lock-file .#devShells.x86_64-linux.default.name`
  in the smoke project.
- `nix path-info --derivation --accept-flake-config --no-write-lock-file .#devShells.x86_64-linux.default`
  in the smoke project.
