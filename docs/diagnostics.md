# Diagnostics

`robo doctor` is the primary debugging surface.

It should explain:

- what runtime components were selected
- what is expected from `uv.lock` and project files
- what host prerequisites are missing
- which failure belongs to Python resolution, Nix runtime setup, or host configuration

For CUDA projects, `robo doctor --deep` should keep the ownership boundary clear:

- Nix provides the native build surface: `nvcc`, CUDA headers, CCCL headers, and `libcudart` for linking native extensions.
- `uv.lock` and uv-installed `nvidia-*` wheels provide Python CUDA runtime libraries such as cuBLAS, cuDNN, cuSolver, cuSparse, and NCCL.
- The NVIDIA host driver provides `libcuda.so.1`; it is not vendored by `robo-nix`.

If `uv.lock` selects CUDA wheels newer than the host NVIDIA driver supports, `robo doctor` and `robo run` should fail before Python reaches simulator startup. Errors from Warp or Isaac such as missing CUDA driver entry points usually belong to the host driver layer or to the `libcuda.so.1` that the runtime loader found, not to Nix or uv.

For editable Python packages that build native extensions, keep the layer boundary the same:

- `pyproject.toml` and uv decide which Python build helpers are installed, whether build isolation is enabled, and which dependency groups are synced.
- `robo shell` points uv at a Nix-provided CPython executable for the requested Python major/minor version, so uv-created virtualenvs run with a libc and dynamic loader that are ABI-aligned with Nix runtime libraries on older distros.
- Nix provides the compiler, CMake, pkg-config, headers, and native libraries selected by components such as `native-build`, `media`, and `cuda-toolkit`.
- `robo shell` exposes CMake package prefixes from uv's `.venv` when those packages provide `share/cmake`, so CMake-based builds can find project-owned Python helpers such as pybind11 without patching vendor sources.
- `robo shell` removes Nix-managed Python include and library flags from native build variables, so setuptools and PyTorch extensions use the uv interpreter's own Python headers and ABI.

If `uv.lock` installs Python wheels that provide native tool shims such as `cmake`, `ninja`, or `patchelf`, those executables are still Python-owned package contents. Project bootstrap scripts that build C/C++ code should use the runtime's native tools instead of `.venv/bin` shims, especially on older distro containers where wheel binaries can mix host glibc with Nix runtime libraries.

If a package fails with `Findpybind11.cmake`, `pybind11Config.cmake`, or another missing CMake package from the Python layer, the scalable fix is usually project-owned: add the helper package to the relevant uv group and disable build isolation for the package that expects the active environment. Do not patch vendored setup files unless the upstream package itself is wrong.

If a virtualenv was created before entering `robo shell`, recreate it from the runtime. Host-created Python environments can load Nix `libstdc++`, graphics, or FFmpeg libraries with the host distro glibc, which can fail on older containers with errors such as `GLIBC_2.38 not found`.

Project bootstrap scripts are project-owned code. Non-interactive `robo init` records discovered bootstrap scripts as review suggestions instead of enabling them automatically. A project enables bootstrap only by adding scripts to the `bootstrap` block in `robo.nix` or by passing `--source-script` explicitly. If bootstrap fails, fix the project script or its required environment variables rather than adding project-specific policy to `robo-nix`.

For graphics projects, `robo doctor --deep` reports the observed display and EGL/GLVND state from the realized runtime shell:

- active session variables such as `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, and `DISPLAY`
- the selected `libEGL.so.1`
- the selected `__EGL_VENDOR_LIBRARY_FILENAMES` entries and whether they exist
- a warning when Nix `libEGL.so.1` is paired with a non-Nix EGL vendor file

This check is diagnostic only. `robo` should not scan host driver directories or guess NVIDIA/Mesa vendor paths during shell setup.
