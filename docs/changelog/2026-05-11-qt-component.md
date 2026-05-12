# 2026-05-11 - Qt Component

## Goal

Make Qt6 a first-class runtime component so projects can request Qt CMake and
runtime support with `"qt6"` instead of spelling Nixpkgs package attributes in
`extraPackages` and `extraRuntimeLibraries`.

## Conflict Check

- Keep package-specific CMake config ownership explicit. `native-build` should
  continue to provide generic compiler/CMake tooling, not silently infer Qt for
  every native build.
- A `"qt6"` component is an explicit `robo.nix` addition and keeps the Nixpkgs
  package details inside robo-nix.
- Keep the component narrow: Qt6 base plus Core5Compat covers common robot
  vendor services that need `Qt6Config.cmake`, `Qt6::Core`,
  `Qt6::Network`, and `Qt6::Core5Compat`.

## Failure Observed

A downstream XRoboToolkit CMake configure step failed with:

```text
Could not find a package configuration file provided by "Qt6"
```

Adding `pkgs.qt6.qtbase` and `pkgs.qt6.qt5compat` manually fixed the CMake
configure/build path, but the manifest was not user-friendly.

## Scope

- Add `qt6` to the known runtime components.
- Wire `qt6` to Qt6 base and Qt6 Core5Compat build/runtime inputs.
- Document `qt6` as the user-facing way to request Qt CMake files, Qt tools,
  plugins, and runtime libraries.
- Update the downstream project manifest to use `"qt6"` instead of raw Nixpkgs
  package attributes.

## Non-Goals

- Do not make `native-build` auto-add package-specific CMake configs.
- Do not add a broad Qt module registry.
- Do not rewrite existing user manifests from `robo shell`.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] Render a temporary project with `qt6` and verify CMake finds Qt6 Core,
  Network, and Core5Compat.
- [x] Verify XRoboToolkit PC service configure/build with a downstream manifest
  that uses `"qt6"`.
