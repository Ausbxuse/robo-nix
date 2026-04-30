# Diagnostics

`robo check` is the primary debugging surface.

It should explain:

- what runtime components were selected
- what is expected from `uv.lock` and project files
- what host prerequisites are missing
- which failure belongs to Python resolution, Nix runtime setup, or host configuration

For CUDA projects, `robo check --deep` should keep the ownership boundary clear:

- Nix provides the native build surface: `nvcc`, CUDA headers, CCCL headers, and `libcudart` for linking native extensions.
- `uv.lock` and uv-installed `nvidia-*` wheels provide Python CUDA runtime libraries such as cuBLAS, cuDNN, cuSolver, cuSparse, and NCCL.
- The NVIDIA host driver provides `libcuda.so.1`; it is not vendored by `robo-nix`.

If `uv.lock` selects CUDA wheels newer than the host NVIDIA driver supports, `robo check` and `robo run` should fail before Python reaches simulator startup. Errors from Warp or Isaac such as missing CUDA driver entry points usually belong to the host driver layer or to the `libcuda.so.1` that the runtime loader found, not to Nix or uv.

TODO(robo): move stable probe logic from `robo-cli` into `robo-checks`.
