# Iteration 004 - Linux Input Headers

## Trigger

A downstream robot-learning project failed during `uv sync` while building
`evdev==1.9.2`.
`evdev` generates and compiles native extensions from Linux input headers, so
the failure belongs to the Nix-owned native/runtime layer.

## Pre-Fix Reproducer

After review correction, reproduced the failure against parent commit
`8a551626d7cc3b4dcd306126173d4027163c75a9`, which is the state before this
iteration's fix.

Temporary project:

- `.python-version`: `3.11`
- `pyproject.toml` dependencies: `torch` and
  `evdev<1.9.3; sys_platform == 'linux'`

The old bootstrap generated only:

```nix
components = [
  "python-uv"
  "native-build" # inferred from pyproject.toml: torch
];
```

Failing command:

```bash
nix develop --accept-flake-config --no-write-lock-file path:. --command sh -c 'uv venv "$UV_PROJECT_ENVIRONMENT" && uv pip install --no-cache --python "$UV_PROJECT_ENVIRONMENT/bin/python" "evdev==1.9.2"'
```

Key error:

```text
The 'linux/input.h' and 'linux/input-event-codes.h' include files are missing.
```

## Scope

- Add a minimal `linux-headers` component backed by `pkgs.linuxHeaders`.
- Export `ROBO_NIX_LINUX_HEADERS`, `CPATH`, and `C_INCLUDE_PATH` so isolated uv
  package builds can find `linux/input.h`, `linux/input-event-codes.h`, and
  `linux/uinput.h`.
- Infer both `native-build` and `linux-headers` from a first-bootstrap
  `pyproject.toml` dependency on `evdev`.
- Keep existing `robo.nix` canonical. Existing projects should add
  `linux-headers` manually if their `robo.nix` already exists.

## Non-Goals

- No project-specific patching for the downstream project.
- No `uv sync` automation.
- No `robo check` or `robo diagnose` surface.
- No attempt to classify every package that may include Linux kernel headers.

## Review Notes

Pending concerns:

- Existing generated `flake.nix` files from older iterations do not know the
  `linux-headers` component. The rewrite branch still bootstraps missing files
  only and does not repair existing robo flakes.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- Smoke bootstrap in `/tmp/robo-iter4-smoke` using `evdev<1.9.3; sys_platform == 'linux'`.
- `nix-instantiate --parse flake.nix` in the smoke project.
- `nix-instantiate --parse robo.nix` in the smoke project.
- `nix eval --accept-flake-config --no-write-lock-file path:.#devShells.x86_64-linux.default.name`
  in the smoke project.
- `nix path-info --derivation --accept-flake-config --no-write-lock-file path:.#devShells.x86_64-linux.default`
  in the smoke project.
- `nix develop --accept-flake-config --no-write-lock-file path:. --command sh -c ...`
  in the smoke project to confirm `linux/input.h`, `linux/input-event-codes.h`,
  `linux/uinput.h`, `CPATH`, and `C_INCLUDE_PATH` are visible.
- `nix develop --accept-flake-config --no-write-lock-file path:. --command sh -c 'uv venv ... && uv pip install ... evdev==1.9.2'`
  in the smoke project to confirm the failing package builds.
