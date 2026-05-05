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

On NVIDIA systems, desktop OpenGL viewers often need the host NVIDIA GLVND
provider rather than Nix Mesa. For runtimes with `x11-gl`, `robo` probes common
host manifest locations and, when it finds NVIDIA manifests, materializes those
paths into `robo run` and `robo shell`.
If those manifests point at host NVIDIA vendor libraries by soname, `robo`
resolves them through the host linker cache and creates a project-local bridge
directory containing only those vendor libraries and their NVIDIA graphics
dependencies.
This is not an offload launcher; use the host's normal launch policy, such as
`nvidia-offload`, when the machine needs PRIME render offload.

Disable this host graphics bridge with:

```bash
export ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO=1
```

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
- The host owns GPU kernel drivers, display sockets, GPU devices, PRIME/offload, and `libcuda.so.1`.
- For `x11-gl`, `robo` may bridge detected host NVIDIA EGL/Vulkan manifests and a small project-local vendor library directory without owning the offload launcher.
- uv owns Python packages and Python CUDA wheels.
- `robo check graphics --verbose` reports observed graphics and driver state.

This behavior is tied to the explicit graphics component. Non-graphics shells do
not receive host graphics manifests by default.

## Current Limits

The host NVIDIA bridge fixes the common case where an `x11-gl` runtime needs the
host NVIDIA GLVND provider to create a desktop OpenGL context. It does not mean
all graphics setups are solved.

Known gaps:

- AMD and Intel hosts currently rely on the Nix Mesa path. There is no separate
  host-provider bridge for unusual AMD or Intel setups.
- PRIME/offload remains host policy. `robo` may expose NVIDIA libraries, but it
  does not decide whether a command should run on the integrated or discrete GPU.
- Wayland issues are diagnosed, not automatically repaired.
- Headless and remote rendering modes such as EGL-only rendering, OSMesa,
  VirtualGL, and container display forwarding still need explicit project or
  host setup.
- There is no nixGL-style one-command wrapper yet. If a command needs launcher
  behavior instead of shell-wide runtime setup, that is still future work.
  A future wrapper should learn from nixGL's provider detection and launch-time
  environment model, but should still document what remains host policy.

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

This is diagnostic. For runtimes that need a host NVIDIA graphics provider, `robo` only materializes detected manifest files from known host locations plus a project-local bridge for the resolved vendor libraries, and leaves user-provided graphics variables unchanged.
