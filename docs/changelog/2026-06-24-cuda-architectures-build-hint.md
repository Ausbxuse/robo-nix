# CUDA Architectures Build Hint

## Context

`devenv` exposes nixpkgs CUDA architecture configuration for packages built by
Nix. `robo-nix` had no equivalent runtime manifest surface. For uv-based
robot-learning projects, the common gap is narrower: Python packages such as
FlashAttention or local PyTorch extensions may compile CUDA kernels and need a
GPU architecture hint such as `8.9`.

## Review Ledger

Related prior concerns:

- `2026-05-08-minimal-cuda-and-gl-runtime.md` added the `cuda-toolkit`
  component but explicitly avoided a CUDA wheel/driver compatibility solver.
- `2026-05-09-environment-isolation-and-cuda-host-bridge.md` keeps host CUDA
  driver bridging narrow and Rust-owned.

No conflict blocks a small manifest hint. This change does not make robo a CUDA
package builder and does not add package-specific CUDA workarounds.

## Change

- Add optional `cudaArchitectures` to `robo.nix` profile manifests.
- Accept `null`, `"auto"`, or an explicit list such as `[ "8.6" "8.9" ]`.
- Export common build hints for CUDA extension builds:
  `TORCH_CUDA_ARCH_LIST`, `CMAKE_CUDA_ARCHITECTURES`, and `CUDAARCHS`.
- Keep auto-detection best-effort and non-fatal.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] temporary project generated `flake.nix` and `robo.nix` parse
- [x] temporary project with `cudaArchitectures = [ "8.9" ];` exports
      `TORCH_CUDA_ARCH_LIST=8.9`, `CMAKE_CUDA_ARCHITECTURES=89`, and
      `CUDAARCHS=89`
