# 2026-05-09 - Local Runtime Graph Inference

## Goal

Make first-bootstrap runtime inference robust for dependencies declared through
the root `pyproject.toml` default metadata graph, including local path
dependencies, and report where static inference stops.

## Conflict Check

- `uv` still owns package resolution, dependency groups, extras, virtualenv
  sync, and lockfiles. `robo shell` must not become a Python resolver or run
  `uv sync` during bootstrap.
- Runtime inference remains first-bootstrap only. Existing `robo.nix` stays
  canonical and is not rewritten.
- Inference rules remain data-driven through `src/metadata/runtime-inference.tsv`.
- The previous non-goal of "no Python resolver" still stands. This change
  only reads local metadata that the root `pyproject.toml` statically points at.

## Scope

- Parse dependency extras from requirement strings such as
  `robot-stack[full]`.
- Read `[tool.uv.sources]` path entries for local source dependencies.
- Follow local path dependencies recursively when their `pyproject.toml` is
  available, including extras selected by the parent requirement.
- Infer components from packages discovered through that local metadata walk.
- Emit first-bootstrap attention diagnostics when a dependency cannot be
  inspected because it has no local source, a local path is missing, or a local
  `pyproject.toml` is invalid or absent.

## Non-Goals

- No network metadata fetches.
- No lockfile mutation or lockfile-as-authority behavior.
- No attempt to emulate uv's full marker, extras, conflict, or version solving.
- No automatic mutation of an existing `robo.nix`.

## Verification

- [x] `cargo test --no-default-features`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] `npm --prefix docs run build`
