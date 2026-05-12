# 2026-05-09 - Native Build Shell Ergonomics

## Goal

Make the native build environment easier to inspect and faster to re-enter
without adding downstream-specific workarounds.

## Conflict Check

- `native-build` already owns compiler tools and common native runtime
  libraries.
- Downstream packages and scripts still own their build metadata and install
  logic.
- Exposing a generic libc development prefix is acceptable; injecting
  dependency-specific paths or mutating downstream files is not.
- Shell freshness should fingerprint the environment that `robo` actually
  launches, not the parent process environment before runtime preparation.
- A shell environment cache is acceptable when it is keyed by real runtime
  inputs and invalidates when referenced Nix store paths disappear.
- Reinstalling `robo` from a local checkout through `nix profile add .#robo`
  should make newly generated downstream project flakes use that installed
  source without extra environment variables.

## Failure Observed

A downstream install script was run outside `robo shell` and failed while
importing a native Python package:

```text
ImportError: libstdc++.so.6: cannot open shared object file
```

Inside `robo shell`, the same native package import succeeded, but the script
then failed an environment sanity check:

```text
missing libc development headers path
```

The shell had selected `native-build`, and the wrapped compiler could find libc
headers, but the shell did not expose a stable `robo-nix` variable for the libc
development prefix.

The first prompt in a new interactive shell also re-evaluated the dev shell
because launch-time runtime fingerprints were computed before host CUDA and
library path preparation mutated the environment.

## Scope

- Export a generic `ROBO_NIX_LIBC_DEV` path when `native-build` is selected.
- Keep the value tied to the Nix compiler's libc development output.
- Treat the variable as robo-managed so active shell refresh can update or unset
  it correctly.
- Compute active shell fingerprints from the launched environment after runtime
  preparation.
- Cache the captured Nix runtime environment under `.robo-nix/`, keyed by runtime
  inputs, and validate referenced `/nix/store` paths before reusing it.
- Embed the Nix-built package source URL as the default generated `robo-nix`
  input, while keeping `ROBO_NIX_DEFAULT_SOURCE_URL` as an explicit override.
- Filter the embedded package source so repo-local caches and heavy generated
  directories are not copied into the Nix store.
- Document the variable as a native-build inspection surface.

## Non-Goals

- No downstream script changes.
- No package-specific build fixes.
- No automatic CUDA toolkit selection for manually invoked installer scripts.
- No persistent GC root or profile management for cached shells.

## Verification

- [x] `cargo test --no-default-features`
- [x] `cargo build --no-default-features`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] local shell smoke confirms `ROBO_NIX_LIBC_DEV` is exported with
  `native-build`
- [x] temporary-project smoke confirms the second run uses the runtime environment
  cache and still exports `ROBO_NIX_LIBC_DEV`
- [x] Nix package build embeds a local source URL in newly generated downstream
  `flake.nix`
- [x] built package closure references the embedded source store path
- [x] embedded source excludes repo-local cache and generated directories
