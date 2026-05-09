# Iteration 020 - CMake Diagnostics And Venv Freshness

## Goal

Improve the product-side handling of native Python build failures without
turning `robo-nix` into a Python package resolver or masking downstream build
metadata bugs.

## Conflict Check

- `uv` still owns Python packages and build dependencies.
- `native-build` owns compiler tools, CMake availability, and common native
  runtime libraries.
- `robo-nix` must not auto-inject package-specific CMake paths or install
  Python-owned build helpers.
- Existing `robo.nix` remains user-managed and is not rewritten.

## Failure Observed

A downstream editable package failed during `uv sync --all-groups` while
running CMake from a Python build backend. The key error was:

```text
Could not find a package configuration file provided by "SomePackage"
```

The package had the relevant Python dependency installed in the uv environment,
but its CMake build did not pass `SomePackage_DIR` or `CMAKE_PREFIX_PATH` to the
directory containing `SomePackageConfig.cmake`.

## Scope

- Keep CMake behavior unchanged.
- Add a generic `native-build` diagnostic wrapper around CMake configure
  failures that recognizes missing CMake package config errors and points users
  at downstream `*_DIR`/`CMAKE_PREFIX_PATH` fixes.
- Track the default `.venv/bin/python` in active shell freshness so shells
  refresh after uv creates or replaces the project virtual environment.
- Document missing CMake package configs as a distinct failure mode from missing
  shared libraries.

## Non-Goals

- No package-specific CMake path injection.
- No Python package installation through Nix.
- No Python dependency graph solving.
- No downstream-specific workaround in `robo.nix`.

## Verification

- [x] `cargo test --no-default-features`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] local CMake diagnostic smoke with a temporary project using a deliberately
  missing CMake package config
