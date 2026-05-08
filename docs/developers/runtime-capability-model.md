# Runtime Capability Model

This page records the current capability model for keeping runtime inference scalable. It is not a standing plan; incomplete behavior should stay marked as incomplete near the code or user-facing docs it affects.

The design borrows Pixi's strongest idea, not Pixi's environment model: detected machine facts and declared environment requirements should be separate objects that can be compared clearly. In Pixi, those objects are Conda virtual packages such as `__cuda` and `__glibc`. In `robo-nix`, they should be uv/Nix runtime capabilities.

## Goal

Prefer this flow:

```text
project facts -> runtime requirements -> providers -> diagnostics
```

Avoid this flow:

```text
project facts -> components
```

Components are an implementation detail. Requirements are the user-facing contract.

## Ownership

- uv owns Python version selection, `.venv` creation/sync, Python packages, and `uv.lock`.
- Nix owns the CPython interpreter, native/runtime libraries, compilers, CUDA/graphics/ROS/simulator tooling, and shell environment.
- The host owns kernel drivers, GPU devices, `libcuda.so.1`, display servers, and system services.
- `robo` owns fact collection, requirement inference, provider matching, and diagnostics.

## Project Facts

Project facts are observations. They should not directly select Nix components.

Examples:

```text
pyproject dependency: mujoco
uv.lock wheel: torch 2.7.0+cu128
uv.lock package: nvidia-cudnn-cu12
workspace path: third_party/isaac-sim
workspace file: setup.py containing CUDAExtension
```

Facts may come from `pyproject.toml`, `uv.lock`, workspace scans, generated `robo.nix`, or explicit user overrides.

## Runtime Requirements

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
- `source`: where it came from
- `reason`: concise user-facing explanation
- `evidence`: optional concrete file/package/version
- `severity`: `required` or `suggested`
- `version`: optional constraint for versioned requirements

## Providers

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
```

Nix components must not claim host-owned capabilities. For example, `cuda-toolkit` must not provide `host.cuda.driver` or `host.cuda.libcuda`.

When a project requires host CUDA and the CLI can detect `host.cuda.libcuda`,
`robo` may materialize that host provider into the cached runtime environment.
That bridge is still host-owned: users can override it with
`ROBO_NIX_LIBCUDA_PATH` or disable automatic bridging with
`ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1`.

Some simulators also need host graphics manifests, not just Nix OpenGL libraries.
For `isaac-sim`, the CLI may materialize detected host NVIDIA EGL and Vulkan
manifest files into the cached runtime environment. When those manifests name
host vendor libraries by soname, the CLI may resolve them through the host
linker cache and append only those vendor library directories. That bridge does
not own PRIME/offload policy; users still launch with their host's normal
mechanism when needed. Disable it with `ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO=1`.

## Metadata Shape

The metadata remains Nix. The important boundary is semantic: rules produce requirements, and components declare capabilities.

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

CUDA Python wheels do not imply `runtime.cuda.toolkit` by themselves. They usually carry user-space CUDA libraries through `uv.lock` and `nvidia-*` wheels, while the host driver still provides `libcuda.so.1`.

## CLI Behavior

`robo init` should:

1. collect project facts
2. infer runtime requirements
3. choose Nix components that provide runtime-owned requirements
4. write the selected components into `robo.nix`

`robo check` should:

1. load the selected runtime components and project facts
2. probe host capabilities
3. read selected Nix component providers
4. compare requirements against providers
5. print grouped diagnostics

Implemented today:

- `nix/metadata/runtime-inference.nix` rules can emit `requires`.
- `nix/metadata/components.nix` components declare `provides`.
- `robo init` keeps generated `robo.nix` focused on editable selections and resolves runtime-owned requirements to components.
- `robo check --why` and `robo contract` expose the requirement contract.

Still incomplete:

- Host providers are diagnosed by focused checks, but there is not yet one unified provider comparison engine for every host requirement.
- Low-confidence findings, such as workspace CUDA marker scans, remain suggestions until the user promotes them.
