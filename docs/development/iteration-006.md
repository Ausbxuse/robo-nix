# Iteration 006 - GLFW Wayland Keyboard Library

## Trigger

After the native build/runtime fixes, a downstream MuJoCo runtime reached GLFW
initialization and failed while GLFW initialized its Wayland backend.

## Pre-Fix Reproducer

Focused command:

```bash
UV_PROJECT_ENVIRONMENT=/tmp/robo-downstream-verify-venv UV_CACHE_DIR=/tmp/robo-downstream-verify-uv-cache robo run python -c 'import glfw; print(glfw.init())'
```

Key error:

```text
GLFWError: (65544) b'Wayland: Failed to load libxkbcommon'
```

Observed result:

```text
0
```

## Scope

- Add `pkgs.libxkbcommon` to `desktop-gl`.
- Keep this in the graphics component because GLFW's Wayland backend needs it
  for keyboard/layout support.

## Non-Goals

- No app-specific downstream workaround.
- No broader host graphics detection.
- No change to host NVIDIA graphics or CUDA driver policy.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Upgrade installed `robo`.
- Regenerate downstream runtime files from installed `robo`.
- `UV_PROJECT_ENVIRONMENT=/tmp/robo-downstream-verify-venv UV_CACHE_DIR=/tmp/robo-downstream-verify-uv-cache robo run python -c 'import glfw; print(glfw.init())'`
  in the downstream project.

Post-fix result:

- The focused GLFW command returned `1` with no `Failed to load libxkbcommon`
  warning.
