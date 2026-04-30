# Runtime Capability Model

This is the target design for keeping `robo-nix` scalable.

The design borrows Pixi's strongest idea, not Pixi's product model: detected
machine facts and declared environment requirements should be separate objects
that can be compared clearly. In Pixi, those objects are Conda virtual packages
such as `__cuda` and `__glibc`. In `robo-nix`, they should be uv/Nix runtime
capabilities.

## Goal

`robo-nix` should stop growing direct package-to-component guesses as the main
extension path.

Prefer this flow:

```text
project facts -> runtime requirements -> providers -> diagnostics
```

Avoid this flow:

```text
project facts -> components
```

Components are an implementation detail. Requirements are the user-facing
contract.

## Ownership

- uv owns Python versions, `.venv`, Python packages, and `uv.lock`.
- Nix owns native/runtime libraries, compilers, CUDA/graphics/ROS/simulator
  tooling, and shell environment.
- The host owns kernel drivers, GPU devices, `libcuda.so.1`, display servers,
  and system services.
- `robo` owns fact collection, requirement inference, provider matching, and
  diagnostics.

## Object Model

### Project Facts

Project facts are observations. They should not directly select Nix components.

Examples:

```text
pyproject dependency: mujoco
uv.lock wheel: torch 2.7.0+cu128
uv.lock package: nvidia-cudnn-cu12
workspace path: third_party/isaac-sim
workspace file: setup.py containing CUDAExtension
```

Facts may come from `pyproject.toml`, `uv.lock`, workspace scans, generated
`robo.nix`, or explicit user overrides.

### Runtime Requirements

Runtime requirements describe what must be true for the project to run.

Examples:

```text
host.cuda.driver >= 12.8
host.cuda.libcuda
runtime.cuda.toolkit
runtime.cuda.nvcc
runtime.cuda.headers
runtime.cuda.link.cudart
runtime.graphics.opengl
runtime.graphics.x11
runtime.native.compiler
runtime.native.libstdcxx
runtime.media.ffmpeg
runtime.sim.mujoco
runtime.sim.isaac
```

Each requirement should carry:

- `id`: stable machine-readable name
- `source`: where it came from, such as `uv.lock`, `pyproject.toml`, or
  `workspace`
- `reason`: concise user-facing explanation
- `evidence`: optional concrete file/package/version
- `severity`: `required` or `suggested`
- `version`: optional constraint for versioned requirements

### Providers

Providers satisfy runtime requirements.

There are two provider classes:

- host providers, detected by probes
- Nix component providers, declared in metadata

Examples:

```text
host probe: nvidia-smi reports CUDA 12.6
  provides host.cuda.driver = 12.6

host probe: ldconfig finds libcuda.so.1
  provides host.cuda.libcuda

component cuda-toolkit
  provides runtime.cuda.toolkit
  provides runtime.cuda.nvcc
  provides runtime.cuda.headers
  provides runtime.cuda.link.cudart

component x11-gl
  provides runtime.graphics.opengl
  provides runtime.graphics.x11

component native-build
  provides runtime.native.compiler
  provides runtime.native.libstdcxx
```

Nix components must not claim host-owned capabilities. For example,
`cuda-toolkit` must not provide `host.cuda.driver` or `host.cuda.libcuda`.

## Metadata Shape

The metadata can remain Nix for now. The important change is semantic: rules
produce requirements, and components declare capabilities.

Example inference rule:

```nix
{
  dependencies = ["mujoco"];
  requires = [
    "runtime.sim.mujoco"
    "runtime.graphics.opengl"
    "runtime.graphics.x11"
    "runtime.native.libstdcxx"
  ];
  reason = "MuJoCo loads native simulator and graphics libraries";
}
```

Example CUDA wheel rule:

```nix
{
  lockPackages = ["torch"];
  versionSuffix = "+cu128";
  requires = [
    {
      id = "host.cuda.driver";
      minVersion = "12.8";
    }
    "host.cuda.libcuda"
  ];
  reason = "torch cu128 wheels require a CUDA 12.8-compatible host driver";
}
```

CUDA Python wheels do not imply `runtime.cuda.toolkit` by themselves. They
usually carry user-space CUDA libraries through `uv.lock` / `nvidia-*` wheels,
while the host driver still provides `libcuda.so.1`.

Example component metadata:

```nix
cuda-toolkit = {
  category = "gpu";
  description = "CUDA compiler, headers, and native extension build support.";
  provides = [
    "runtime.cuda.toolkit"
    "runtime.cuda.nvcc"
    "runtime.cuda.headers"
    "runtime.cuda.link.cudart"
  ];
};
```

## CLI Behavior

### `robo init`

`robo init` should:

1. collect project facts
2. infer runtime requirements
3. choose Nix components that provide runtime-owned requirements
4. write the selected components and the requirement contract into `robo.nix`

It should not pretend host-owned requirements are solved by Nix.

### `robo check`

`robo check` should:

1. load the requirement contract
2. probe host capabilities
3. read selected Nix component providers
4. compare requirements against providers
5. print grouped diagnostics

Example output shape:

```text
requirements
  ok     CUDA toolkit link surface     cuda-toolkit
  error  CUDA driver >= 12.8           found 12.6
  ok     CUDA driver library           /run/opengl-driver/lib/libcuda.so.1
  ok     OpenGL runtime                x11-gl
  ok     native C++ runtime            native-build

why
  torch cu128 wheels require a CUDA 12.8-compatible host driver
  MuJoCo loads native simulator and graphics libraries

next
  upgrade the NVIDIA driver
  or regenerate uv.lock with CUDA wheels supported by this host
```

## Migration Plan

1. Add `provides` to `nix/metadata/components.nix`.
2. Add `requires` to inference rules while keeping existing `components` for a
   short internal transition.
3. Teach Rust to infer requirements and derive components from provider
   metadata.
4. Store the requirement contract in generated `robo.nix` provenance.
5. Move CUDA, graphics, and native-build checks to compare requirements against
   host/runtime providers.
6. Remove direct package-to-component inference once generated projects no
   longer need it.

This does not require a SAT solver. A deterministic set-cover style selection is
enough at first: include every component that is the default provider for an
unsatisfied runtime-owned required capability.

## Non-goals

- Do not integrate Pixi as a backend.
- Do not replace `uv.lock` with `pixi.lock`.
- Do not create a central Python package registry.
- Do not infer project-specific uv groups, extras, source pins, or package
  indexes.
- Do not let Nix components claim host driver ownership.
