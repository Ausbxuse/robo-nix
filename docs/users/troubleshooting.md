# Troubleshooting

Start with `robo check`:

```bash
robo check
```

It reports what `robo` observed, which layer likely owns the failure, and what to inspect next.

## Which Command To Use

- The project seems broken right now: `robo check`
- The failure area is obvious: `robo check cuda`, `robo check graphics`, `robo check native`, or `robo check python`
- You need slower runtime probes: `robo check --deep`
- You already have a traceback, compiler log, or loader error: `robo diagnose -`
- You need component provenance: `robo check --why`

Examples:

```bash
robo check graphics
robo check cuda --verbose
robo check --deep

uv sync 2>&1 | robo diagnose -
robo diagnose build.log
```

`robo diagnose` classifies existing error text. It does not probe the current machine and it does not apply fixes.

## Common Failures

Search for a distinctive phrase from the error log.

### `GLIBC_2.38 not found`

Usually means the active Python environment was created outside the `robo` runtime and is mixing host Python/glibc with Nix native libraries.

Try:

```bash
robo shell
uv venv --clear
uv sync
```

### `libcuda.so.1` or `CUDA driver library not found`

Usually means the host NVIDIA driver or `libcuda.so.1` is not visible to the runtime. Nix can provide CUDA build tools, but the proprietary driver comes from the host.

Try:

```bash
robo check cuda --verbose
```

If the driver is installed in a non-standard location:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

### CUDA wheel and driver mismatch

If `uv.lock` selects CUDA wheels newer than the host driver supports, the Python lock or host driver must change.

Examples:

```text
host supports CUDA 12.6, uv.lock expects CUDA 12.8
CUDA driver version is insufficient
```

Upgrade the NVIDIA driver, or regenerate the project's lockfile with CUDA wheels compatible with the host.

### `Qt6Config.cmake` missing

Usually means a native CMake build needs Qt development/runtime files that are not in the runtime.

Try:

```bash
robo add qt6
robo up
```

### `pybind11Config.cmake` or `nanobindConfig.cmake` missing

Usually means the project expects a Python package such as `pybind11` or `nanobind` to provide CMake files, but that package is not available in the active uv environment or build isolation hides it.

Use the uv command documented by the project. If you maintain the project, declare the helper package in `pyproject.toml`.

### EGL or OpenGL context failure

Examples:

```text
EGL: Failed to get EGL display
gladLoadGL error
an OpenGL platform library has not been loaded
Wayland: Failed to load libwayland-client
```

Usually means the graphics runtime cannot create an OpenGL context. The cause may be display socket visibility, EGL vendor files, GLVND loader state, Wayland/X11 access, or host/container graphics integration.

Try:

```bash
robo check graphics --verbose
```

### Missing local editable source

Examples:

```text
Distribution not found at: file:///.../third_party/...
No such file or directory
```

Usually means the project's Python dependency metadata points at a local source checkout that is missing.

Try:

```bash
git submodule update --init --recursive
```

If the project uses a different vendor-source policy, follow that project's docs. `robo` should not guess a project-specific install mode.

### Missing Linux headers

Examples:

```text
linux/input.h: No such file or directory
linux/joystick.h: No such file or directory
```

Usually means a native extension includes Linux kernel userspace headers.

Try:

```bash
robo add linux-headers
robo up
```

## Failure Ownership

When something fails, classify the layer first:

- `uv` / Python: dependency resolution, groups, extras, lockfile, editable sources, package indexes, native Python build backend behavior
- Nix runtime: missing native package, compiler, shared library, ROS/simulator component, generated shell state
- host: GPU driver, display server, devices, container permissions, system services
- project bootstrap: project-owned scripts and local SDK setup
- `robo` CLI: confusing workflow, poor diagnostics, wrong generated files, incorrect command wrapping

`robo` makes that classification easier. It does not silently convert one layer's failure into another layer's hidden workaround.
