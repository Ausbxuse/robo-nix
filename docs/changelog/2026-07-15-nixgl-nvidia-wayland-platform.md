# 2026-07-15 - nixGL NVIDIA Wayland Platform

## Goal

Make the bundled `nixGLNvidia` wrapper expose its Nix-packaged Wayland EGL
external-platform provider. Keep hybrid-GPU PRIME selection project-owned.

## Conflict Check

- nixGL wrappers own graphics wrapper variables; robo must not add PRIME
  render-offload defaults on top of wrapper output.
- Nix-managed desktop client libraries remain separate from host graphics
  policy.
- Do not scan host driver directories or the Nix store for graphics plugins.

The existing runtime fallback scans NVIDIA driver outputs for external-platform
files. Current nixpkgs packages `egl-wayland` separately, so the durable fix is
to patch the bundled nixGL wrapper from its `pkgs` package set and remove the
scan rather than expand it.

## Failure Observed

In a Wayland session on a hybrid AMD/NVIDIA workstation, an operator profile
with `hostGraphics = "nixgl-nvidia"` failed its native PyGLFW window path:

```text
GLFWError: (65542) b'EGL: Failed to get EGL display: Success'
python: glfwSetKeyCallback: Assertion `window != NULL' failed.
```

The failing bounded command was:

```text
robo run -p operator -- python -c '<create a 64x64 GLFW window>'
```

The wrapper selected `libEGL_nvidia`, but
`__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS` was unset. The NVIDIA driver output did
not contain the manifest because nixpkgs provided it under the separate
`pkgs.egl-wayland` output. Supplying that package's manifest and library plus
the project's explicit `__NV_PRIME_RENDER_OFFLOAD=1` created a native Wayland
window rendered by the NVIDIA GPU.

## Scope

- Patch robo's bundled nixGL source so the NVIDIA wrapper exports the
  `pkgs.egl-wayland` external-platform manifest directory.
- Include the matching `egl-wayland` library in the wrapper's library path.
- Remove robo's NVIDIA-output and Nix-store fallback scan.
- Verify the generated operator environment and a native Wayland GLFW window.

## Non-Goals

- Do not set `__NV_PRIME_RENDER_OFFLOAD` or GLX offload variables from robo.
- Do not force projects from Wayland to X11/XWayland.
- Do not add host graphics directories to the runtime library path.

## Verification

- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test` (110 passed)
- [x] `nix flake check --impure`
- [x] downstream operator profile evaluated with
      `--override-input robo-nix path:/path/to/robo-nix`
- [x] native PyGLFW Wayland window rendered by the NVIDIA RTX 5090
- [x] bounded `we teleop --profile keyboard --max-seconds 1.5` reached
      `operator window ready` and stopped cleanly
