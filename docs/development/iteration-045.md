# Iteration 045 - nixGL NVIDIA Compatibility

## Goal

Keep explicit `hostGraphics = "nixgl-nvidia"` usable with the current pinned
nixpkgs NVIDIA derivation interface.

## Conflict Check

- Keep host graphics policy explicit and let nixGL own non-NixOS graphics
  wrapping.
- Do not change default `hostGraphics = "auto"` behavior.
- Do not restore robo-owned GLX/EGL/GBM host graphics wrapping.
- Keep `ROBO_NIX_NVIDIA_VERSION` as the explicit override for driver-version
  detection gaps.

No active review-ledger conflict blocks a small compatibility patch around the
bundled nixGL source.

## Reproduction

After changing `robo.nix` in an active runtime shell, refresh failed while
building the explicit NVIDIA nixGL wrapper:

```bash
nix-build --no-out-link /nix/store/f2nqm9j29r48lzay38vn827gwgsd55c7-source -A auto.nixGLNvidia --argstr nvidiaVersion 590.48.01 --arg enable32bits false
```

Key error:

```text
error: function 'anonymous lambda' called with unexpected argument 'kernel'
at .../pkgs/os-specific/linux/nvidia-x11/generic.nix:34:1
```

The bundled nixGL source still passes `kernel = null` when overriding
`nvidia_x11` for `libsOnly`; the current pinned nixpkgs NVIDIA generic package
no longer accepts that argument.

## Scope

- Patch the bundled nixGL source inside Nix before building `nixGLNvidia`.
- Use that patched source only for explicit `hostGraphics = "nixgl-nvidia"`.
- Build the patched nixGL source against robo's pinned nixpkgs input rather
  than the user's ambient `<nixpkgs>`.
- Keep importing only nixGL-selected runtime variables.

## Non-Goals

- Do not change generic nixGL wrapper selection for `auto` or `nixgl`.
- Do not add host graphics scans.
- Do not vendor a separate graphics wrapper implementation.

## Verification

- [x] reproduced failing `nix-build`
- [x] patched-source `nix-build` for `nixGLNvidia-590.48.01`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] temporary project `flake.nix` and `robo.nix` parse
- [x] temporary `hostGraphics = "nixgl-nvidia"` project enters `nix develop`
      with `ROBO_NIX_NVIDIA_VERSION=590.48.01`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix fmt -- --check src/nix/project-flake.nix`
