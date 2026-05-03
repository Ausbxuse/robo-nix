# Graphics

Graphics support is selected through runtime components such as `x11-gl` and `qt6`.

Use them for:

- MuJoCo and simulator viewers
- desktop OpenGL
- GLFW and EGL
- Qt applications and Qt-based build dependencies
- OpenCV GUI paths
- plotting backends that need native display libraries

## X11, Wayland, and EGL

`x11-gl` provides Nix-managed userspace graphics libraries and points EGL at a matching Nix Mesa GLVND vendor file by default.

That default prevents a common broken state: loading Nix `libEGL.so.1` while EGL reads a distro or container vendor file from `/usr/share/glvnd`.

Users may still override `__EGL_VENDOR_LIBRARY_FILENAMES` when a host-specific EGL vendor is required, but that should be a deliberate host fix.

## Qt

`qt6` provides Qt 6 runtime and development packages.

It uses standard environment surfaces such as:

- `CMAKE_PREFIX_PATH`
- `QT_PLUGIN_PATH`

Projects do not need robo-specific variables to find Qt. If a project script requires a custom variable for a Qt prefix, that is usually project-specific glue that belongs in that project.

## Why robo-nix Does Not Behave Like nixGL by Default

`nixGL` is a launcher wrapper. On NVIDIA systems it can read the host driver version, assemble matching NVIDIA userspace graphics libraries, set GLVND/EGL/Vulkan environment variables, and run one graphical command.

`robo shell` is broader than a single graphical launcher. It is where users run:

- uv
- Python installs
- native extension builds
- CMake
- simulators
- Qt tools
- ROS tools
- project bootstrap scripts

Injecting host NVIDIA graphics paths into the whole shell can fix one viewer while changing dynamic loader behavior for unrelated Python and C++ build steps.

The `robo-nix` default keeps the boundary explicit:

- Nix components expose coherent runtime libraries.
- The host owns GPU kernel drivers, display sockets, GPU devices, and `libcuda.so.1`.
- uv owns Python packages and Python CUDA wheels.
- `robo check graphics --verbose` reports observed graphics and driver state.

If proprietary NVIDIA OpenGL/EGL support becomes a common requirement, it should be added as an explicit component or mode with its own diagnostics, not as hidden default shell setup.

## Debugging

Use:

```bash
robo check graphics --verbose
```

For graphics projects, deep diagnostics report:

- `XDG_SESSION_TYPE`
- `WAYLAND_DISPLAY`
- `DISPLAY`
- selected `libEGL.so.1`
- selected EGL vendor file entries
- whether EGL vendor files exist
- warnings when Nix `libEGL.so.1` is paired with non-Nix vendor files

This is diagnostic. `robo` does not guess driver paths during shell setup.
