# Cached Shell Host CUDA Latency

## Context

In `we-teleop`, a cached runtime shell printed:

```text
✓ robo ready                                             0ms
  └ ✓ runtime cache                      cached          0ms
```

but still paused before launching the interactive shell. Timing showed:

```text
nvidia-smi --query-gpu=driver_version --format=csv,noheader  3.311 total
ROBO_NIX_DEBUG=1 robo run true                              3.310 total
zsh -i -c exit                                              0.255 total
```

The post-cache host CUDA report called `nvidia-smi` only to record a diagnostic
driver version. That blocked cached shell/run startup even when
`ROBO_NIX_LIBCUDA_PATH` was already known from the cached runtime environment.

## Review Ledger

Related prior concerns:

- `2026-05-09-environment-isolation-and-cuda-host-bridge.md` keeps host CUDA
  driver bridging narrow and avoids broad host driver policy.
- `2026-05-11-host-nvidia-version-detection.md` allows NVIDIA version probing
  for `hostGraphics = "nixgl-nvidia"` so nixGL can be built with the matching
  driver version.

No conflict blocks this change. Host CUDA bridging does not need the NVIDIA
driver version; host graphics can keep probing when the selected graphics
policy requires nixGLNvidia.

## Change

- Stop calling `nvidia-smi` from the Rust host CUDA bridge/report path.
- Keep host CUDA detection based on explicit `ROBO_NIX_LIBCUDA_PATH`,
  `LD_LIBRARY_PATH`, `ldconfig`, and known driver library locations.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `we-teleop` cached runtime smoke with checkout binary completes in
      about 0.6s instead of matching the 3.3s `nvidia-smi` timing
