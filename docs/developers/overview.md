# Developer Overview

This branch is a greenfield rebuild. Keep the product surface small and
reviewable.

## Boundaries

- `robo shell` prepares and enters the runtime.
- `robo run` prepares the same runtime, then runs one command.
- uv owns `pyproject.toml`, dependency groups, extras, sync policy, and the
  Python virtualenv.
- Nix owns CPython, native tools, runtime libraries, and shell environment.
- Rust owns CLI flow, diagnostics, templates, and command wrapping.

## Generated Files

`robo shell` may create `flake.nix` and `robo.nix` during first bootstrap.

After `robo.nix` exists, it is user-managed and canonical. Shell must not rewrite
it.

`desktop-gl` is Nix-managed desktop graphics support. `cuda-toolkit` is the
Nix-owned CUDA build toolkit. Host CUDA drivers remain host-owned; this branch
only honors an explicit `ROBO_NIX_LIBCUDA_PATH` and does not scan host driver
directories.

Generated text should live in `templates/` or `metadata/` and be embedded with
`include_str!`.

## Iterations

Each iteration should:

- Start from the review ledger.
- Call out conflicts before coding.
- Keep diffs narrow.
- Update `AGENTS.md` only for durable rules.
- Add focused verification notes to `docs/development/iteration-*.md`.
