# Roadmap

This roadmap is intentionally conservative. `robo-nix` should become easier to use by tightening the product boundary, not by absorbing every downstream project policy.

## Short Term

- keep `robo up`, `robo check`, `robo diagnose`, `robo shell`, `robo run`, and `robo status` reliable
- make first-run setup feel beginner-friendly without hiding Python package installation policy
- keep runtime inference data-driven in `nix/metadata`
- keep Python ownership in uv
- improve native/runtime diagnostics for common robotics failures
- keep docs truthful about shipped behavior and known limits

## Later

- split stable Rust boundaries only when the codebase needs it
- add real templates only after common downstream usage is proven
- package the CLI through normal distribution paths without making Python packaging depend on Nix
- evolve runtime inference toward the capability model
- add explicit proprietary NVIDIA graphics support only if it can be documented and diagnosed as a clear opt-in mode

## Not Current Goals

- Nix-managed Python as a first-class mode
- central Python package registry or preset matrix
- hidden host driver path scanning
- project-specific uv group or extra selection
- task runner support before the runtime workflow is solid
