# 2026-05-09 - Simulator Runtime Library Coverage

## Goal

Cover generic runtime libraries needed by large simulator stacks without adding
downstream-specific package workarounds.

## Conflict Check

- `desktop-gl` owns desktop graphics and windowing runtime libraries.
- `native-build` already carries common native runtime libraries, not only
  compilers.
- Host NVIDIA driver policy remains separate from Nix-managed graphics
  libraries. Vulkan/EGL host-driver selection is not added automatically in this
  iteration.

## Failure Observed

A downstream Isaac Sim smoke test started Kit but logged missing shared
libraries from bundled simulator extensions:

```text
libXt.so.6: cannot open shared object file
libGLU.so.1: cannot open shared object file
libcrypt.so.1: cannot open shared object file
```

The same run also failed to create an RTX/Vulkan device and repeatedly queried
CUDA device ordinal `-1`. That is tracked as host NVIDIA Vulkan/renderer policy,
not as a generic desktop library dependency.

## Scope

- Add `libXt` and `libGLU` to the `desktop-gl` runtime surface.
- Add legacy `libcrypt.so.1` through `libxcrypt-legacy` to `native-build`.
- Keep NVIDIA Vulkan ICD selection explicit for now.

## Non-Goals

- No automatic host NVIDIA Vulkan/EGL probing.
- No `/usr/bin` compatibility layer for tools such as `nvidia-smi` or `lscpu`.
- No Isaac Sim-specific component.

## Verification

- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] local override smoke confirms `libXt.so.6`, `libGLU.so.1`, and
  `libcrypt.so.1` are visible
