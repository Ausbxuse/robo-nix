# Architecture

`robo-nix` has four active implementation layers:

- `crates/robo-cli`: command-line UX, diagnostics, command wrapping, and generated project files
- `nix/modules`: reusable runtime component implementations
- `nix/metadata`: component metadata, starter profiles, runtime inference rules
- `nix/mk-flake.nix`: turns a project manifest into flake outputs

## Data Flow

The intended flow is:

```text
project files -> observed facts -> runtime requirements -> Nix components -> diagnostics
```

Avoid this as the long-term extension model:

```text
project files -> hardcoded Rust heuristics -> components
```

Rust should stay boring and generic: read manifests, scan project files, apply metadata rules, call Nix/uv, explain results.

## Generated Projects

Generated projects use `robo-nix.lib.mkProjectFlake` through `flake.nix`.

The main project contract is `robo.nix`. It should stay small enough for downstream users to read and maintain.

Generated files should be regenerated from source logic rather than hand-edited during development:

```bash
robo init . --force
```

## Runtime Components

Components are implementation units in `nix/modules`.

They should expose reusable runtime capability, such as:

- `python-uv`
- `native-build`
- `media`
- `x11-gl`
- `qt6`
- `cuda-toolkit`
- `mujoco`
- `ros2-jazzy`

Do not add a component for a single downstream project unless the behavior is clearly reusable.

## Host Boundaries

Do not treat host path inventories as a scalable abstraction.

For GPU and graphics, prefer explicit diagnostics over generated-shell scans. Host driver visibility should be diagnosed from observed environment/tool output, not guessed from arbitrary filesystem inventories.

When a runtime contract clearly requires a host-owned capability, `robo` may materialize a narrowly detected provider into the cached runtime environment. Current examples are `libcuda.so.1` for CUDA-wheel and Isaac Sim runtimes, and NVIDIA EGL/Vulkan manifest files plus their resolved vendor library directories for Isaac Sim. These bridges must be opt-out, respect user-provided environment variables, and must not own host policy such as PRIME/offload launchers.

If a generic exported fact is needed, design the contract first, document it, and add focused validation.
