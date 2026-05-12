# 2026-05-09 - Environment Isolation And CUDA Host Bridge

## Goal

Fix final-process environment isolation, expand first-bootstrap inference to
cover common dependency locations, and port the reviewed host CUDA driver bridge
shape from `develop` into the rewrite branch without restoring broader command
surfaces.

## Conflict Check

The active repository guidance says not to add generated-shell scans over host
CUDA, NVIDIA, EGL, Vulkan, WSL, or distro driver directories. The requested
change explicitly asks for the `develop` branch style driver probing. Treat that
as a reviewed exception for this change and keep it narrow:

- probe from explicit environment, inherited library path, `ldconfig`, and the
  known host locations used on `develop`;
- do not add interactive shell prompts or generated-shell directory scans;
- keep `ROBO_NIX_LIBCUDA_PATH` as an override and add an opt-out variable;
- materialize only the CUDA driver library bridge, not host EGL/Vulkan policy.

Existing review-ledger concerns remain valid:

- prompt refresh is export-only and may leave removed variables in active
  shells;
- comment-only `robo.nix` edits still trigger refresh;
- search candidate ranking remains simple.

## Scope

- Clear the inherited process environment before applying the captured Nix
  dev-shell environment to `robo shell` and `robo run`.
- Add regression coverage that a variable absent from the captured environment
  cannot leak into the launched process.
- Extend `pyproject.toml` inference to read `[project].optional-dependencies`,
  `[dependency-groups]`, and legacy `[tool.uv].dev-dependencies` arrays in
  addition to `[project].dependencies`.
- Add observed robot-learning dependency rules from the `develop` branch where
  they map to components that exist in this rewrite branch.
- Add Rust host CUDA bridge logic: detect when a project likely needs host
  `libcuda.so.1`, find a visible host provider, and append runtime exports.
- Apply the host CUDA bridge during initial shell/run environment preparation
  and prompt-time refresh.
- Update user/developer docs for exact environment application, expanded
  inference, automatic host CUDA bridge, override, opt-out, and failure
  signatures.

## Non-Goals

- No `robo init`, `robo check`, or `robo diagnose` restoration.
- No CUDA wheel/driver compatibility solver.
- No robo-owned host NVIDIA EGL/Vulkan graphics wrapping.
- No new root Node tooling.
- No automatic `uv sync`.

## Verification

Ran for this change:

- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix flake check --accept-flake-config`
- `npm --prefix docs run build`
- `nix-instantiate --parse flake.nix`
- `nix-instantiate --parse src/nix/project-flake.nix`
- `nix-instantiate --parse src/templates/project/flake.nix`
- temporary project render with optional dependencies and dependency groups,
  followed by `nix-instantiate --parse` on its generated `flake.nix` and
  `robo.nix`
