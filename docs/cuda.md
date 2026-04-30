# CUDA

CUDA support is split between host reality and reproducible runtime setup.

- `uv.lock` pins Python wheels.
- `robo` can infer the expected CUDA wheel ABI from `uv.lock`.
- Nix provides the CUDA toolkit when the `cuda-toolkit` component is selected.
- The host NVIDIA driver remains a host prerequisite.
- `robo` detects host driver CUDA support by loading NVML first, then falling back to `nvidia-smi`.
- For CUDA runtimes, `robo activate` exposes the host driver library by checking `ROBO_NIX_LIBCUDA_PATH`, common Linux/NixOS driver locations, and `ldconfig`. `ROBO_NIX_LIBCUDA_PATH` may point either to `libcuda.so.1` or to the directory containing it.

Python CUDA wheels and the CUDA toolkit are checked separately:

- CUDA wheels in `uv.lock` require a compatible host NVIDIA driver and visible `libcuda.so.1`.
- The `cuda-toolkit` component provides `nvcc`, CUDA headers, CCCL headers, and `libcudart` link support for native extension builds.
- A project with PyTorch CUDA wheels does not need `cuda-toolkit` unless it builds native CUDA extensions.

Run:

```bash
robo check
```

TODO(robo): make CUDA version selection more interactive once the host-driver compatibility policy is fully specified.
