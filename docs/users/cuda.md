# CUDA

CUDA support is split across uv, Nix, and the host.

This split is important because no single layer owns the whole CUDA stack.

## Ownership

uv owns Python CUDA wheels:

- `torch` wheels
- `jaxlib` wheels
- `nvidia-*` wheels such as cuBLAS, cuDNN, cuSolver, cuSparse, and NCCL

Nix owns the native CUDA build surface when selected:

- `nvcc`
- CUDA headers
- CCCL headers
- `libcudart` link support
- compiler and linker environment

The host owns:

- NVIDIA kernel driver
- GPU devices
- `libcuda.so.1`
- driver-supported CUDA API version

Nix does not vendor the proprietary host driver. uv wheels do not install the kernel driver. `robo` reports this boundary clearly.

## Common Cases

Python-only CUDA wheels usually need a compatible host driver but not the CUDA toolkit:

```text
torch + cu128 wheel -> host driver compatible with CUDA 12.8
```

Native CUDA extension builds need the toolkit component:

```text
CUDAExtension, .cu files, custom kernels -> cuda-toolkit
```

Run:

```bash
robo doctor --deep
```

Deep diagnostics report the observed driver state and the CUDA requirements inferred from `uv.lock`.

## Driver Mismatch

If `uv.lock` selects CUDA wheels newer than the host driver supports, fail early. Examples include:

- CUDA 12.8 wheel with a driver that only supports CUDA 12.6
- missing or invisible `libcuda.so.1`
- container without GPU device access

Errors from Warp, Isaac, PyTorch, or JAX about missing CUDA driver symbols usually belong to the host driver layer or to the `libcuda.so.1` the runtime loader found.

## Driver Library Path

`robo shell` does not scan arbitrary host driver directories and mutate library paths. This avoids turning one host-specific fix into a global shell behavior that affects Python installs, C++ builds, and simulators.

If a package needs an explicit CUDA driver path, set:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

or point it at the directory containing `libcuda.so.1`.

`robo doctor --deep` reports what it observes so the user can make that host-owned fix deliberately.
