# Runtime Failure Guide

This page is the start of a runtime failure guide for common robotics setup errors.

It is not a promise that `robo` can fix every project. Many failures belong to project policy, host drivers, private package indexes, vendor SDKs, missing source checkouts, or upstream package bugs. The goal is to make common runtime failures searchable and easier to classify.

## How To Use This Page

Search for a distinctive phrase from the error:

- `GLIBC_2.38 not found`
- `libcuda.so.1`
- `Qt6Config.cmake`
- `gladLoadGL error`
- `EGL: Failed to get EGL display`
- `Distribution not found at`
- `Could not find pybind11Config.cmake`

Then use the ownership boundary to decide what to fix.

## GLIBC Version Not Found

Common snippets:

```text
GLIBC_2.38 not found
GLIBC_2.34 not found
required by /nix/store/.../libstdc++.so.6
```

Usually means:

The active Python environment was created outside the robo runtime and is mixing host Python/glibc with Nix native libraries. This is common in older containers or older Ubuntu hosts.

Owned by:

Python environment state plus native runtime ABI alignment.

Try:

```bash
robo shell
uv venv --clear
uv sync
```

Use the uv command documented by the project if it needs groups or extras.

## Missing Local Editable Source

Common snippets:

```text
Distribution not found at: file:///.../third_party/...
No such file or directory
```

Usually means:

The project's Python dependency metadata points at a local source checkout that is missing.

Owned by:

Project checkout, submodules, vendored sources, or dependency declarations.

Try:

```bash
git submodule update --init --recursive
```

If the project uses a different vendor-source policy, follow that project's docs. `robo` should not guess a project-specific install mode.

## Qt CMake Package Missing

Common snippets:

```text
Could not find Qt6Config.cmake
Could not find a package configuration file provided by "Qt6"
```

Usually means:

A native CMake build needs Qt development/runtime files that are not in the robo runtime.

Owned by:

Nix runtime dependencies.

Try:

```bash
robo add qt6
robo up
```

Then rerun the project-owned build command.

## Python-Owned CMake Helper Missing

Common snippets:

```text
Could not find pybind11Config.cmake
Could not find nanobindConfig.cmake
```

Usually means:

The project expects a Python package such as `pybind11` or `nanobind` to provide CMake files, but that package is not available in the active uv environment or build isolation hides it.

Owned by:

Project Python dependency policy.

Try:

Use the uv command documented by the project. If you maintain the project, make sure build requirements and uv groups are declared clearly.

## CUDA Driver Not Visible

Common snippets:

```text
libcuda.so.1: cannot open shared object file
CUDA driver library not found
CUDA_ERROR_NO_DEVICE
```

Usually means:

The host NVIDIA driver or `libcuda.so.1` is not visible to the runtime. Nix can provide CUDA build tools, but the proprietary driver comes from the host.

Owned by:

Host GPU driver integration.

Try:

```bash
robo check cuda --verbose
```

If the driver is installed in a non-standard location, set:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

or point it at the directory containing `libcuda.so.1`.

## CUDA Wheel And Driver Mismatch

Common snippets:

```text
host supports CUDA 12.6, uv.lock expects CUDA 12.8
CUDA driver version is insufficient
```

Usually means:

Python CUDA wheels selected by `uv.lock` require a newer NVIDIA driver API than the host provides.

Owned by:

Host driver version or project Python dependency lock.

Try:

Upgrade the NVIDIA driver, or regenerate the project's lockfile with CUDA wheels compatible with the host.

## EGL Or OpenGL Context Failure

Common snippets:

```text
EGL: Failed to get EGL display
gladLoadGL error
an OpenGL platform library has not been loaded
Wayland: Failed to load libwayland-client
```

Usually means:

The graphics runtime cannot create an OpenGL context. The cause may be display socket visibility, EGL vendor files, GLVND loader state, Wayland/X11 access, or host/container graphics integration.

Owned by:

Host graphics/display integration plus selected runtime graphics libraries.

Try:

```bash
robo check graphics --verbose
```

## Missing Linux Headers

Common snippets:

```text
linux/input.h: No such file or directory
linux/joystick.h: No such file or directory
```

Usually means:

A native extension includes Linux kernel userspace headers.

Owned by:

Nix runtime dependencies.

Try:

```bash
robo add linux-headers
robo up
```

## Native Build Tool Shim Mixing

Common snippets:

```text
.venv/bin/cmake
.venv/bin/ninja
version `GLIBC_...` not found
```

Usually means:

A Python wheel installed an executable build-tool shim. That shim may run outside the ABI boundary expected by the runtime.

Owned by:

Project build invocation plus Python/native boundary.

Try:

Prefer native tools from the robo runtime for C/C++ builds:

```bash
robo shell
which cmake
which ninja
```

If a Python package requires its own tool shim, treat the failure as project-specific and keep the workaround in the project.
