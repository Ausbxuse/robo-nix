# Troubleshooting

Start from the project directory with `robo check`:

```bash
robo check
```

It reports what `robo` observed, which layer likely owns the failure, and what to inspect next.

## Which Command To Use

- The project seems broken right now: `robo check`
- The failure area is obvious: `robo check cuda`, `robo check graphics`, `robo check native`, or `robo check python`
- You need slower runtime probes: `robo check --deep`
- You already have a traceback, compiler log, or loader error: `robo diagnose -`
- You need to know why a runtime piece was selected: `robo check --why`

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

### GLIBC symbol error

Usually means the active Python environment was created outside the `robo` runtime and is mixing host Python/glibc with Nix native libraries.

From the project directory, enter the runtime shell and recreate the virtualenv:

```bash
robo shell
uv venv --python "$ROBO_NIX_PYTHON" --clear
uv sync
```

### Python environment missing or host-owned

Examples:

```text
Python virtualenv is missing
Python virtualenv was created outside robo-nix
existing uv environment was not created from the current robo-nix Python
```

Usually means `robo` can see the runtime contract, but the project `.venv` is missing, was created with a host Python interpreter, or still points at an older robo-nix Python after the runtime input graph changed.

From the project directory, enter the runtime shell and sync explicitly:

```bash
robo shell
uv sync
```

If the virtualenv was created outside `robo`, recreate it with the runtime interpreter. The same fix applies when `robo shell` prints an expected `ROBO_NIX_PYTHON` path that differs from `.venv/bin/python`.

```bash
robo shell
uv venv --python "$ROBO_NIX_PYTHON" --clear
uv sync
```

### Python project files missing

Examples:

```text
.python-version is missing
pyproject.toml is missing
uv.lock missing
```

Usually means the project-owned Python contract is incomplete. `uv` owns Python package metadata, dependency locking, and virtualenv sync; `robo` expects those files so it can provide the matching native/runtime layer.

For a new project, run:

```bash
robo init .
uv sync
```

For an existing project, restore or create the missing project file according to that project's Python policy.

### Runtime files or components need review

Examples:

```text
runtime files need review
runtime components may be incomplete
required directories missing
```

Usually means generated runtime files are stale, the selected `robo.nix` components do not match project metadata, or `robo.nix` declares workspace directories that are not present in this checkout.

From the project directory, refresh generated files and review `robo.nix`:

```bash
robo init . --force
robo check
```

If `requiredDirectories` is wrong, create the missing directories or edit that list in `robo.nix`.

### Runtime shell tool missing

Examples:

```text
uv is not available in the runtime shell
failed to probe uv in runtime shell
```

Usually means the Nix runtime shell did not expose a tool that `robo` expects to provide.

From the project directory, rebuild and run the deep check:

```bash
robo build
robo check --deep
```

### Native build tool shims in `.venv`

Examples:

```text
Python virtualenv contains native build tool shims: cmake
Python environment contains native build tool shims: ninja
```

Usually means Python packages installed command shims such as `cmake`, `ninja`, or `patchelf` into `.venv/bin`. Python may own helper packages, but Nix should own compilers, CMake, Ninja, pkg-config, and native build tools used by project bootstrap or native extension builds.

From inside the runtime, confirm the active tools:

```bash
robo shell
which cmake
which ninja
```

If project scripts call `.venv/bin/cmake` or `.venv/bin/ninja`, change those scripts to use the runtime tools on `PATH`.

### CUDA driver library not found

Usually means the host NVIDIA driver or `libcuda.so.1` is not visible to the runtime. Nix can provide CUDA build tools, but the proprietary driver comes from the host.

From the project directory, ask `robo` what it can see:

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

### CUDA toolkit not visible

Examples:

```text
CUDA root is not visible in the current shell
CUDA native build surface is incomplete
CUDA_HOME/CUDA_PATH did not point at a toolkit
```

Usually means a project is building native CUDA extensions and needs the Nix-owned CUDA toolkit surface: `nvcc`, headers, CCCL headers, and the `libcudart` link surface.

Add `cuda-toolkit` to `components` in `robo.nix`, then run:

```bash
robo check --deep
```

### Qt CMake files missing

Usually means a native CMake build needs Qt development/runtime files that are not in the runtime.

From the project directory, add `qt6` to `components` in `robo.nix`, then prebuild or re-enter the runtime:

```bash
robo build
# or enter the runtime again with:
robo shell
```

### Python CMake helper missing

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

From the project directory, ask `robo` what graphics state it can see:

```bash
robo check graphics --verbose
```

The verbose graphics check reports display socket visibility, `libEGL`, EGL vendor files, `/dev/dri`, container graphics hints, host graphics bridge state, and OpenGL renderer evidence when a renderer probe is available. For MuJoCo runtimes with a synced `.venv`, it also tries a small MuJoCo OpenGL context. If the renderer is `llvmpipe`, `softpipe`, or another software rasterizer, fix host/container GPU visibility before debugging simulator code.

### Python GUI import failed

Examples:

```text
PyQt6 GUI import failed
matplotlib QtAgg backend probe failed
```

Usually means the Python GUI package is installed but the runtime does not have the needed Qt, display, or OpenGL/EGL support.

Run the deep check after syncing Python dependencies:

```bash
uv sync
robo check --deep
```

For desktop GUI backends, review `qt6` and `desktop-gl` in `robo.nix`.

### FFmpeg media runtime missing

Examples:

```text
TorchCodec import failed
TorchCodec needs FFmpeg shared libraries
```

Usually means a Python video/media package needs FFmpeg shared libraries from the Nix runtime.

Add `media` to `components` in `robo.nix`, then run:

```bash
robo check --deep
```

### Missing local editable source

Examples:

```text
Distribution not found at: file:///.../third_party/...
No such file or directory
```

Usually means the project's Python dependency metadata points at a local source checkout that is missing.

From the repository root, fetch the source checkout if the project uses Git submodules:

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

From the project directory, add `linux-headers` to `components` in `robo.nix`, then prebuild or re-enter the runtime:

```bash
robo build
# or enter the runtime again with:
robo shell
```

### Missing shared library

Examples:

```text
ImportError: libassimp.so.5: cannot open shared object file: No such file or directory
error while loading shared libraries: libusb-1.0.so.0
```

Usually means the Python package installed correctly, but the Nix runtime is missing a native shared library.

From the project directory, ask `robo` for packages indexed by `nix-locate`:

```bash
robo search libassimp.so.5
```

If the local package-file index is missing or stale, run:

```bash
nix-index
```

If the result is a narrow library, add it to `extraRuntimeLibraries` in `robo.nix`:

```nix
extraRuntimeLibraries = pkgs: [
  pkgs.assimp
];
```

If the result points at a broader component such as `media`, `desktop-gl`, or `native-build`, prefer adding that component when the project needs the broader runtime contract.

If you only need a command such as `ffmpeg`, `imagemagick`, or another CLI tool on `PATH`, use `extraPackages` instead. `extraRuntimeLibraries` is for shared libraries that Python packages load at runtime.

## Failure Ownership

When something fails, classify the layer first:

- `uv` / Python: dependency resolution, groups, extras, lockfile, editable sources, package indexes, native Python build backend behavior
- Nix runtime: missing native package, compiler, shared library, ROS/simulator component, generated shell state
- host: GPU driver, display server, devices, container permissions, system services
- project bootstrap: project-owned scripts and local SDK setup
- `robo` CLI: confusing workflow, poor diagnostics, wrong generated files, incorrect command wrapping

`robo` makes that classification easier. It does not silently convert one layer's failure into another layer's hidden workaround.
