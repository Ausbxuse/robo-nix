# Iteration 018 - Runtime Decision Spine

## Goal

Turn the rewrite branch into a release-ready runtime shell by borrowing the
durable engineering patterns from Pixi, uv, and devenv without expanding
`robo-nix` into a package manager or general environment manager.

## Conflict Check

- `robo shell` remains the canonical workflow; do not restore `robo init`,
  `robo check`, or `robo diagnose`.
- `uv` continues to own Python package metadata, dependency groups, extras,
  virtualenv sync, and lockfiles.
- `Nix` continues to own CPython, native tools, runtime libraries, and shell
  environment construction.
- `Rust` owns command UX, diagnostics, runtime decision tracing, project-file
  preparation, and command wrapping.
- Existing dirty worktree changes are preserved. This iteration builds on the
  environment isolation, inference, and host CUDA bridge work from iteration
  017.
- Host CUDA probing remains narrow: explicit `ROBO_NIX_LIBCUDA_PATH`, captured
  `LD_LIBRARY_PATH`, `ldconfig`, and the reviewed known host driver locations.
  Do not add broad generated-shell scans over host CUDA/NVIDIA/EGL/Vulkan/WSL
  directories.

## Source Survey Commitments

- Pixi inspiration: explicit runtime states, activation cache invalidation by
  real inputs, shell-specific activation handling, reporter lifecycle, and
  warnings around unsafe host/runtime boundaries.
- uv inspiration: env-var registry as source of truth, hermetic CLI tests,
  snapshot-style output contracts, warn-once behavior, lock timeout diagnostics,
  and actionable operational hints.
- devenv inspiration: component-owned Nix behavior, assertions/warnings, shell
  unsets, generated-file ownership, deterministic runtime state directories,
  and version skew warnings.

Concrete source patterns used:

- Pixi's activation and workspace code keeps shell state explicit and keyed by
  real inputs. This iteration mirrors that by fingerprinting runtime files plus
  runtime-affecting env vars, and by treating refresh as a shell-specific delta.
- uv's env-var registry, hermetic integration tests, and lock-timeout behavior
  informed the public env registry, documentation contract test, project lock
  timeout, and redacted `.robo-nix/last-run.json` artifact.
- devenv's Nix modules keep component-owned behavior close to the Nix layer and
  emit unsets for removed shell variables. This iteration keeps component env
  exports in `src/nix/project-flake.nix`, adds `ROBO_NIX_COMPONENTS`, and makes
  refresh unset previously managed variables that disappear.

## Implementation Checklist

- [x] Record source-survey decisions in this iteration doc.
- [x] Add an env-var registry and ensure public env knobs are documented.
- [x] Convert runtime inference from bare package-to-component rows into a
  richer rule schema with capabilities, provenance, and validation.
- [x] Expose dependency evidence from `[project].dependencies`,
  `[project.optional-dependencies]`, `[dependency-groups]`, and legacy
  `[tool.uv].dev-dependencies`.
- [x] Replace host CUDA side effects with explicit facts, probe results, and
  decisions.
- [x] Add a diagnostic-only host NVIDIA driver probe when available.
- [x] Write a versioned `.robo-nix/last-run.json` with redacted facts,
  decisions, env names, warnings, and errors.
- [x] Protect first bootstrap and host CUDA bridge updates with narrow lock
  files and useful timeout errors.
- [x] Make active shell refresh emit unsets for removed robo-managed variables.
- [x] Add component-owned diagnostics and debug output without adding new user
  commands.
- [x] Add focused tests for env isolation, inference evidence, rule validation,
  host CUDA decisions, refresh unsets, debug JSON, and lock behavior.
- [x] Update docs for debug artifacts, env vars, host CUDA ergonomics, and
  issue-reporting workflow.
- [x] Verify with Rust tests, formatting, Nix parse/checks, docs build, and a
  local NVIDIA probe when available.

## Non-Goals

- No Python resolver, dependency synchronizer, or package installer behavior.
- No global robot-learning package registry.
- No automatic mutation of existing `robo.nix`.
- No broad host graphics-driver bridge beyond the reviewed `libcuda` boundary.
- No new root Node tooling.
- No CI wiring to legacy deleted tests.

## Verification

- [x] `cargo test --no-default-features` - 44 tests passed.
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] `nix-instantiate --parse src/templates/project/flake.nix`
- [x] `nix flake check --accept-flake-config`
- [x] `npm --prefix docs run build`
- [x] temporary project render and parse of generated `flake.nix`/`robo.nix`
- [x] local NVIDIA probe: `nvidia-smi` reports NVIDIA GeForce RTX 5090 Laptop
  GPU with driver 595.71.05. `ldconfig -p` has no `libcuda.so.1`, but
  `/run/opengl-driver/lib/libcuda.so.1` exists.
- [x] local host CUDA bridge smoke: a temporary project with CUDA wheel evidence
  in `uv.lock` ran `robo run sh -c ...` successfully and exported
  `ROBO_NIX_LIBCUDA_PATH=/run/opengl-driver/lib/libcuda.so.1`.
