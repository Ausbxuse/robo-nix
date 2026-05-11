# Iteration 031 - Refresh Stale Store Paths

## Goal

Refresh active runtime shells when robo-managed Nix store paths disappear,
even if runtime input files and policy environment variables did not change.

## Conflict Check

- Active `robo shell` sessions should refresh at the next prompt when the
  runtime environment is stale.
- Runtime cache reuse already validates referenced Nix store paths; active shell
  refresh should have the same failure mode.
- Keep refresh transparent. Do not silently paper over project build-system
  cache problems that persist after a fresh runtime environment is exported.

## Failure Observed

In an active runtime shell, a downstream CMake configure step received compiler
paths from the environment:

```text
/nix/store/95k9rsn1zsw1yvir8mj824ldhf90i4qw-gcc-wrapper-14.3.0/bin/cc
/nix/store/95k9rsn1zsw1yvir8mj824ldhf90i4qw-gcc-wrapper-14.3.0/bin/c++
```

CMake reported that those absolute paths were not existing compiler tools.
Because the active runtime input fingerprint had not changed, the shell prompt
refresh hook did not re-evaluate the runtime environment.

## Scope

- Inspect robo-managed active shell variables for referenced `/nix/store/...`
  paths.
- Treat missing referenced store paths as a refresh reason alongside changed
  runtime input files.
- Keep the notice explicit by listing missing store paths before exporting the
  refreshed environment.
- When the local CMake wrapper observes a cached compiler path that no longer
  exists, print a direct hint to remove the affected build directory or
  `CMakeCache.txt`.

## Non-Goals

- Do not clear downstream CMake build directories or `CMakeCache.txt`.
- Do not change compiler component selection.
- Do not add host compiler fallback behavior.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
