# Iteration 025 - uv Venv Targeting and Lockfile Inference

## Goal

Keep `robo shell` easy for normal uv workflows: users should not need to pass
`--python "$UV_PROJECT_ENVIRONMENT/bin/python"` for common `uv pip install`
commands, and runtime inference should use already-resolved lockfile facts when
available.

## Conflict Check

- uv still owns virtualenv sync, dependency groups, extras, and lockfiles.
- Nix still owns the CPython interpreter used to create project environments.
- Reading `uv.lock` as static package evidence is not dependency resolution and
  must not fetch remote metadata.

## Failure Observed

A project had `UV_PROJECT_ENVIRONMENT=.venv-custom`, but
`uv pip install flash_attn --no-build-isolation` tried to install into the
Nix-store CPython because `UV_PYTHON` pointed at the Nix interpreter. The same
project also failed to infer `linux-headers` because `evdev` was only visible as
a resolved transitive package in `uv.lock`.

## Scope

- Wrap uv in the `python-uv` component so `uv pip install` targets
  `$UV_PROJECT_ENVIRONMENT/bin/python` when that venv exists and the user did
  not pass an explicit target.
- Keep upstream uv behavior for explicit targets such as `--python`, `--active`,
  `--system`, `--target`, and `--prefix`.
- Align `VIRTUAL_ENV` with `UV_PROJECT_ENVIRONMENT` for uv subprocesses when
  that environment exists.
- Disable virtualenv prompt rewrites in runtime shells so copied activation
  commands do not duplicate the `[robo]` prompt marker.
- Read `uv.lock` package names as static inference evidence.

## Non-Goals

- No Python dependency resolution in `robo`.
- No mutation of `pyproject.toml`, `uv.lock`, or existing `robo.nix`.

## Verification

- [x] `cargo test --no-default-features`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] temporary generated project confirms `VIRTUAL_ENV_DISABLE_PROMPT=1`
