# Iteration 007 - Native zlib Runtime

## Trigger

After GLFW reached the interactive MuJoCo viewer, `dexmate-teleop` failed during
PyRoki warmup because NumPy could not import in the async worker.

## Pre-Fix Reproducer

Focused command:

```bash
UV_PROJECT_ENVIRONMENT=/home/zhenyu/src/dev/dexmate/dexmate-teleop/.venv UV_CACHE_DIR=/home/zhenyu/src/dev/dexmate/dexmate-teleop/.robo-nix/uv-cache robo run python -c 'import numpy; print(numpy.__version__)'
```

Post-fix result:

- The direct NumPy import printed `2.3.5`.
- The multiprocessing spawn-shaped NumPy import printed `2.3.5` and exited
  with code `0`.

Key error:

```text
ImportError: libz.so.1: cannot open shared object file: No such file or directory
```

The multiprocessing-shaped reproducer showed the same missing library in a
spawned child process.

## Scope

- Add `lib.getLib pkgs.zlib` to the `native-build` runtime library path.
- Keep this as a generic native Python wheel runtime contract.

## Non-Goals

- No NumPy-specific handling.
- No dexmate-specific environment hook.
- No package registry or preset matrix.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Upgrade installed `robo`.
- Regenerate dexmate runtime files from installed `robo`.
- Focused NumPy import in dexmate:

```bash
UV_PROJECT_ENVIRONMENT=/home/zhenyu/src/dev/dexmate/dexmate-teleop/.venv UV_CACHE_DIR=/home/zhenyu/src/dev/dexmate/dexmate-teleop/.robo-nix/uv-cache robo run python -c 'import numpy; print(numpy.__version__)'
```
