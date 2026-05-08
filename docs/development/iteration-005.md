# Iteration 005 - Native C++ Runtime

## Trigger

After iteration 004 fixed the `evdev` build, the real dexmate verification
command moved to the next runtime failure: NumPy could not import because
`libstdc++.so.6` was not visible.

## Pre-Fix Reproducer

Command:

```bash
UV_PROJECT_ENVIRONMENT=/tmp/dexmate-robo-verify-venv UV_CACHE_DIR=/tmp/dexmate-robo-verify-uv-cache robo run ./scripts/teleop.sh --help
```

Observed facts:

- `evdev==1.9.2` built successfully.
- 157 packages installed successfully.
- Importing NumPy failed before help text could print.

Key error:

```text
ImportError: libstdc++.so.6: cannot open shared object file: No such file or directory
```

## Scope

- Add the compiler C++ runtime library to the `native-build` runtime library
  path using `lib.getLib pkgs.stdenv.cc.cc`.
- Keep this as a generic native runtime contract. This is not NumPy-specific.

## Non-Goals

- No package-specific NumPy handling.
- No project-specific dexmate hook.
- No broad diagnostics surface.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Regenerate dexmate runtime files from the installed `robo`.
- `nix eval --accept-flake-config --no-write-lock-file .#devShells.x86_64-linux.default.name`
  in dexmate.
- `UV_PROJECT_ENVIRONMENT=/tmp/dexmate-robo-verify-venv UV_CACHE_DIR=/tmp/dexmate-robo-verify-uv-cache robo run ./scripts/teleop.sh --help`
  in dexmate.
