# Iteration 037 - Host Graphics Boundary Cleanup

## Concern

After adding nixGL support, `hostGraphics = "nixgl-nvidia"` still evaluated
nixGL's auto NVIDIA package and failed before the runtime shell hook:

```text
error: cannot coerce null to a string: null
```

The Rust-owned NVIDIA GLX/EGL/GBM bridge was also redundant with the new nixGL
boundary.

## Decision

- Make `hostGraphics = "auto"` the generated manifest default.
- Resolve `auto` in the generated shell: `/run/opengl-driver` on NixOS and the
  generic nixGL wrapper on other Linux hosts.
- Keep `hostGraphics = null` as the explicit opt-out.
- Treat the old explicit NVIDIA graphics spelling as an alias for
  `hostGraphics = "nixgl-nvidia"` in this iteration.
- Build nixGLNvidia with an explicit host driver version detected at shell
  setup time, avoiding nixGL's failing null auto-detection path.
- Remove the Rust-side NVIDIA graphics wrapping.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
- [x] generated `hostGraphics = "nixgl-nvidia"` smoke test with
  `ROBO_NIX_NVIDIA_VERSION=580.65.06`
