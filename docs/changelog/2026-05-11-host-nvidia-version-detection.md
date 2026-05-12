# 2026-05-11 - Host NVIDIA Version Detection

## Concern

`hostGraphics = "nixgl-nvidia"` still asked users to set
`ROBO_NIX_NVIDIA_VERSION` manually on a host where `nvidia-smi` existed at
`/usr/bin/nvidia-smi`, because the Nix shell did not include `/usr/bin` on
`PATH` during shell-hook evaluation.

## Decision

- Keep `ROBO_NIX_NVIDIA_VERSION` as an override.
- Detect NVIDIA hosts and driver versions with `command -v nvidia-smi`,
  `/usr/bin/nvidia-smi`, `/run/current-system/sw/bin/nvidia-smi`, and
  `/proc/driver/nvidia/version`.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated `hostGraphics = "nixgl-nvidia"` smoke test with `/usr/bin`
  removed from `PATH`
