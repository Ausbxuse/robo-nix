# Testing

`robo-nix` treats environment generation as product code. The test strategy reflects that.

## Test Layers

There are eight main layers.

## 1. Library Regression Tests

Implemented in [tests/regression-api.sh](../tests/regression-api.sh:1).

These tests validate:

- `normalizeEnvSpec` defaults
- component catalog exposure
- platform gating
- repo-maintainer outputs
- `mkProjectFlake` output contract
- `--doctor` success and failure output contracts on CPU-safe environments
- robo init validation errors for invalid generated configurations
- rejection of unknown components

This protects the public API of the flake, not just one preset.

## 2. Downstream Fixture Validation

Implemented in [tests/fixture-validation.sh](../tests/fixture-validation.sh:1).

Fixtures live under [tests/fixtures](../tests/fixtures:1).

These simulate real downstream repositories that depend on `github:ausbxuse/robo-nix` and then get rewritten to a local `path:` reference during local testing.

This catches regressions in the downstream consumption story.

## 3. Project Initializer Validation

Implemented in [tests/robo-init-validation.sh](../tests/robo-init-validation.sh:1).

These tests ensure:

- the interactive helper publishes a reusable component list
- non-interactive generation writes a valid downstream `flake.nix`
- scaffold directories such as `ros_ws/src` are created when required
- generated projects validate through `--doctor`

## 4. Vendor Metadata Validation

Implemented in [tests/vendor-validation.sh](../tests/vendor-validation.sh:1).

These tests validate the curated local vendor index used by `robo vendor list`, `robo vendor add`, `robo vendor doctor`, `robo vendor bootstrap`, and `robo vendor export`. The vendor workflow is local-source-first: it reports known source trees and bootstrap wiring, but it does not fetch proprietary or project-owned vendor code.

## 5. Runtime Contract Validation

Implemented in [tests/contract-validation.sh](../tests/contract-validation.sh:1).

These tests validate the machine-readable `robo contract --json` and `robo doctor --why --json` surfaces. The contract records the generated schema version, selected components, source attribution, derivation name, source input, and lock presence without exposing raw closure implementation details as the user-facing API.

## 6. Output Consistency Validation

Implemented in [tests/output-consistency.sh](../tests/output-consistency.sh:1).

This test blocks raw `doctor:` and `vendor:` print calls in Rust command modules. Human output should use themed label helpers so terminal output stays consistent while captured logs remain grep-friendly.

## 7. Profiling Validation

Implemented in [tests/profile-validation.sh](../tests/profile-validation.sh:1).

This test validates that the built-in profiling command still exists and emits the expected profiled commands. It is not a benchmark assertion; it is a workflow regression check.

## 8. GPU Validation

Implemented in [tests/gpu-validation.sh](../tests/gpu-validation.sh:1), [.github/workflows/gpu-smoke.yml](../.github/workflows/gpu-smoke.yml:1).

This validation is intentionally separate from the default GitHub-hosted CI tier because standard hosted runners do not provide NVIDIA GPUs or CUDA drivers.

The GPU tier is intended for self-hosted runners with labels:

- `self-hosted`
- `linux`
- `x64`
- `gpu`
- `nvidia`

It validates:

- GPU runner availability through `nvidia-smi`
- host CUDA/NVIDIA prerequisites through `nix run .#cuda-doctor`
- `gpu-learning` config output
- dry-run bootstrap for the CUDA-enabled environment
- CUDA-related shell exports inside `nix develop`

## CPU-Safe Vs GPU-Required Checks

The flake now exposes explicit validation-tier checks:

- `checks.<system>.cpu-safe-*`
- `checks.<system>.gpu-required-*`

The distinction is intentional:

- CPU-safe checks are suitable for default GitHub-hosted CI
- GPU-required checks identify entrypoints and GPU-only validation paths

Actual GPU execution still happens in the dedicated `gpu-smoke` workflow or via `nix run .#cuda-doctor` on a self-hosted NVIDIA runner.

## Flake Checks

`nix flake check` validates:

- generated preset checks
- repo lint checks
- output evaluation integrity

The current flake also includes:

- `checks.<system>.lint-nix`
- `checks.<system>.lint-shell`

## Local Commands

Maintainer workflow:

```bash
bash tests/dev-check.sh
```

During active development, prefer the focused script for the surface you touched instead of starting the full suite:

```bash
bash tests/vendor-validation.sh
bash tests/contract-validation.sh
bash tests/output-consistency.sh
bash tests/robo-init-validation.sh
```

Before merging broader changes, run the full validation tier:

```bash
bash tests/full-check.sh
```

`tests/gpu-validation.sh` remains a separate NVIDIA-host check.

For GUI/runtime changes, also smoke the synced downstream project that motivated the change:

```bash
robo run python -c 'from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)'
robo run env MPLBACKEND=QtAgg python -c 'import matplotlib.pyplot as plt; fig = plt.figure(); print(type(fig.canvas).__name__)'
```

The helper form is:

```bash
bash tests/gui-runtime-smoke.sh /path/to/downstream/project
```

Those probes intentionally check the uv-managed Python environment inside the Nix runtime. They catch the common failure where the Python package is installed but a native library such as fontconfig, DBus, or an XCB utility is missing from the runtime library path.

`robo doctor` should stay cheap enough for the normal edit loop. Put checks that may realize large runtime closures behind `robo doctor --deep` or explicit smoke scripts.

## Ubuntu Downstream Smoke Tests

NixOS is not representative of every user host. To test the non-NixOS path, run downstream projects inside an Ubuntu container from the NixOS workstation:

```bash
bash tests/ubuntu-downstream-smoke.sh ~/src/dev/dexmate ~/src/dev/py-learn
```

The helper uses Podman, installs Nix in an Ubuntu container when needed, mounts this checkout as the local `robo-nix` source, copies each downstream project inside the container, and then runs:

```bash
robo init .
robo doctor
robo sync
```

For GUI/runtime regressions, add `--gui`:

```bash
bash tests/ubuntu-downstream-smoke.sh --gui ~/src/dev/dexmate
```
```

This is a manual host-compatibility check, not part of default CI. It needs network access for the first Ubuntu/Nix setup and may realize large runtime closures for downstream projects. Use `--keep` to leave the container around for debugging:

```bash
bash tests/ubuntu-downstream-smoke.sh --keep ~/src/dev/dexmate
podman exec -it robo-nix-ubuntu-smoke bash
```

## CI

The workflow is defined in [ci.yml](../.github/workflows/ci.yml:1).

Ubuntu job:

- formatting check
- full validation wrapper
- root flake checks
- library regression tests
- profiling validation
- vendor metadata validation
- downstream fixture validation
- full robo init validation

macOS job:

- library regression tests
- `nix flake show --all-systems`

## Why This Much Testing

Robotics development environments fail in a few recurring ways:

- evaluation still works but the real shell is broken
- a platform silently disappears
- bootstrap assumptions drift from actual workspace layout
- a public API change breaks downstream projects
- GPU integrations silently regressing because CPU-only CI never exercised them

The current test stack is meant to catch those specific failure modes.

## Testing And AI-Assisted Changes

This repo explicitly allows AI-assisted development, but only under a verifiable workflow.

That means tests and checks are not “nice to have” after AI-generated edits. They are the contract that makes AI usage acceptable here.

For AI-assisted changes:

- run the smallest targeted verification that proves the claim being made
- run the broader maintainer workflow when shared generator logic, presets, project initialization, or docs are affected
- do not merge behavior changes that lack direct evidence

In this repo, verification is the line between useful AI assistance and unreviewable guesswork.
