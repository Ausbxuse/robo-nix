# Iteration 006 - GLFW Wayland Keyboard Library

## Trigger

After the native build/runtime fixes, `dexmate-teleop` launched the MuJoCo
runtime and failed while GLFW initialized its Wayland backend.

## Pre-Fix Reproducer

Focused command:

```bash
UV_PROJECT_ENVIRONMENT=/tmp/dexmate-robo-verify-venv UV_CACHE_DIR=/tmp/dexmate-robo-verify-uv-cache robo run python -c 'import glfw; print(glfw.init())'
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

- No app-specific dexmate workaround.
- No broader host graphics detection.
- No change to `host-nvidia-gl` or CUDA driver policy.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Upgrade installed `robo`.
- Regenerate dexmate runtime files from installed `robo`.
- `UV_PROJECT_ENVIRONMENT=/tmp/dexmate-robo-verify-venv UV_CACHE_DIR=/tmp/dexmate-robo-verify-uv-cache robo run python -c 'import glfw; print(glfw.init())'`
  in dexmate.
