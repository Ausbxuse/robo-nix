# Graphics

Graphics runtime libraries are selected through `x11-gl` and `qt6`.

Use these for desktop OpenGL, Qt, OpenCV GUI paths, simulator viewers, and plotting backends that need native display libraries.

`x11-gl` provides Nix-managed userspace graphics/runtime libraries and points EGL at the matching Nix Mesa GLVND vendor file by default. This keeps Wayland/EGL clients from accidentally mixing Nix `libEGL.so.1` with a distro or container `/usr/share/glvnd` vendor file. Users may still override `__EGL_VENDOR_LIBRARY_FILENAMES` when a host-specific EGL vendor is required.

`qt6` provides Qt 6 runtime and development packages. It exposes Qt through standard `CMAKE_PREFIX_PATH` and `QT_PLUGIN_PATH` so ordinary CMake projects can use `find_package(Qt6 ...)` without robo-specific variables.

`x11-gl` does not scan host-specific NVIDIA driver directories or set host Vulkan paths. If a host driver path is missing, `robo doctor` reports the observed fact and the user-owned environment fix instead of expanding generated shell path guesses.

## Why this is not nixGL by default

`nixGL` is a special-purpose launcher wrapper. For NVIDIA systems it can read the host driver version, assemble matching NVIDIA userspace graphics libraries, set GLVND/EGL/Vulkan environment variables, and then run one graphical command.

`robo shell` is broader than one graphical launcher. It is the environment where users run uv, Python package installs, native extension builds, CMake, simulators, Qt tools, ROS tooling, and project bootstrap scripts. Injecting host NVIDIA graphics paths into that whole shell can fix one viewer while changing the dynamic loader behavior for unrelated Python and C++ build steps.

The `robo-nix` advantage is that the boundary is explicit:

- Nix-provided components expose coherent runtime libraries.
- The host owns GPU kernel drivers, display sockets, GPU devices, and `libcuda.so.1`.
- `uv` owns Python packages and Python CUDA wheels.
- `robo doctor --deep` reports the observed graphics and driver state instead of hiding it behind an implicit wrapper.

This makes failures easier to classify. A missing NVIDIA driver, an invisible `libcuda.so.1`, a bad EGL vendor file, and a Python native-extension ABI mismatch are different problems, and `robo-nix` keeps them in separate layers.

If proprietary NVIDIA OpenGL/EGL support becomes a common requirement, it should be added as an explicit component or mode with its own diagnostics, not as hidden shell setup. That keeps the convenience opt-in while preserving a predictable default runtime for robotics development.
