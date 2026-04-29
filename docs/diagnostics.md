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

TODO(robo): move stable probe logic from `robo-cli` into `robo-checks`.
