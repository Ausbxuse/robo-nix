# Architecture

This document explains how the flake is assembled internally.

## Top-Level Shape

The repo is intentionally split into:

- `components/`: reusable capability definitions
- `envs/`: example preset assemblies
- `lib/`: flake generation logic
- `lib/vendor-modules/`: curated vendor module metadata
- `crates/robo/`: Rust CLI
- `tests/`: regression and validation coverage

## Public API

The public API lives under [lib/default.nix](../lib/default.nix:1).

The main exported functions are:

- `mkFlakeFromEnvCatalog`
- `mkProjectFlake`
- `normalizeEnvSpec`

For most downstream users, `mkProjectFlake` is the only thing they should need.

## Generator Engine

The central engine is [lib/mk-flake-from-envs.nix](../lib/mk-flake-from-envs.nix:1).

Its responsibilities are:

- normalize environment specs
- normalize the uv-managed Python version recorded in the environment spec
- resolve components against a generation context
- merge component contributions
- generate:
  - `apps`
  - `checks`
  - `devShells`
  - `packages`
  - `formatter`
- create default aliases for the chosen default environment

## Output Model

Each environment becomes a generated bundle:

- bootstrap app
- dev shell
- derivation-backed check
- package output for the bootstrap wrapper

The unsuffixed alias points to the default environment.

Example:

- `robot-learning`
- `isaac-ros2-learning`
- `default` -> alias to `robot-learning`

## Validation Model

Validation happens at multiple layers:

- component-level `check` snippets
- workspace file and directory requirements
- uv-managed Python version metadata checks
- profiling workflow validation coverage
- downstream fixture flakes
- robo init validation
- runtime contract validation
- CLI output consistency validation
- vendor workflow validation
- repo-level lint checks

This matters because environment tooling often rots from evaluation still succeeding while real bootstrap behavior breaks.

## Platform Gating

Platform support is explicit.

An environment variant is generated only if:

1. the environment declares the current system in `supportedSystems`
2. every component in the environment also supports that system

This prevents invalid outputs from leaking into unsupported systems.

## Python Boundary

`robo-nix` does not own the Python interpreter or Python packages in the main product path.

Current policy:

- uv owns Python version selection, virtual environments, package resolution, and `uv.lock`
- Nix owns `uv`, native build tools, runtime libraries, CUDA/graphics/ROS/simulator dependencies, and shell environment
- environment specs carry a single `pythonVersion` so `robo init` and `doctor` can keep `.python-version` and user-facing diagnostics aligned

This avoids turning the project into a Python-version matrix or Python package registry.

## Rust CLI Boundary

The Rust CLI owns user-facing workflow:

- `init`
- `doctor`
- `contract`
- `sync`
- `run`
- `develop`
- `vendor`

The CLI should stay boring and generic. Runtime and vendor coverage should come from manifests and Nix/data metadata where possible.

Human CLI output should use themed label helpers. Machine-readable surfaces such as `robo doctor --why --json` and `robo contract --json` must emit raw JSON without colors or labels.

## Generated Project Contract

Generated `robo.nix` files include `schemaVersion = 1` and a `provenance` block.

The provenance block records:

- generator
- selected profile
- component reasons
- inferred notes
- low-confidence suggestions

`robo doctor --why` explains that provenance for humans. `robo doctor --why --json` and `robo contract --json` expose it for CI and audit tooling.

The current contract output is intentionally higher-level than a raw Nix store closure. It is stable enough for project audits while leaving room to add package-version summaries later.

## Vendor Metadata

Vendor metadata is modular. Contributors add focused files under [lib/vendor-modules](../lib/vendor-modules:1). The aggregator [lib/vendor-metadata.nix](../lib/vendor-metadata.nix:1) imports those modules.

Vendor entries use:

- `installPath`
- `detectPaths`
- optional `sourceUrl`
- `components`
- `requiredPaths`
- `bootstrapScripts`
- `patches`

`sourceUrl = null` means the CLI must not fetch that source. This keeps proprietary or project-owned vendor policy local-source-first.

## Why Presets Still Exist

The `envs/` catalog still exists, but only as:

- examples
- validation-covered presets
- a reference for downstream projects

It is not the primary extension mechanism.

## Maintainer Tooling

The top-level flake also exposes repo-maintainer outputs:

- `repo-fmt`
- `repo-lint`

Those exist so the repo’s own workflows are reproducible and can run the same way locally and in CI.

For command development, prefer focused validation first. See [Testing](./testing.md) and [Contributing](../CONTRIBUTING.md).
