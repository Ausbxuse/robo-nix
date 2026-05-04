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
robo check cuda
```

Deep diagnostics report the observed driver state and the CUDA requirements inferred from `uv.lock`.

## Driver Mismatch

If `uv.lock` selects CUDA wheels newer than the host driver supports, fail early. Examples include:

- CUDA 12.8 wheel with a driver that only supports CUDA 12.6
- missing or invisible `libcuda.so.1`
- container without GPU device access

Errors from Warp, Isaac, PyTorch, or JAX about missing CUDA driver symbols usually belong to the host driver layer or to the `libcuda.so.1` the runtime loader found.

## Driver Library Path

For projects that declare CUDA wheels or Isaac Sim, `robo up`
probes the host for `libcuda.so.1`. When it finds a confident provider, such as
`/run/opengl-driver/lib/libcuda.so.1` on many NixOS NVIDIA hosts, `robo run` and
`robo shell` add that driver directory to the runtime automatically.

`robo` does not scan arbitrary host driver directories inside the generated Nix
shell. The CLI materializes a detected host CUDA capability for CUDA projects,
then caches that runtime environment. This keeps the host/Nix/uv boundary clear
without making normal users wire `LD_LIBRARY_PATH` by hand.

To override the detected path, set:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

or point it at the directory containing `libcuda.so.1`.

To disable host CUDA driver auto-bridging, set:

```bash
export ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1
```

## NVIDIA Offload

`nvidia-offload` selects the NVIDIA GPU for rendering on hybrid NixOS systems. It
does not expose the CUDA driver library by itself. Use it when the workload needs
PRIME render offload:

```bash
nvidia-offload robo run python script.py
```

`robo` still handles the `libcuda.so.1` runtime bridge for CUDA projects when the
host driver library is detectable.
