# Graphics

Graphics runtime libraries are selected through `x11-gl` and `qt6`.

Use these for desktop OpenGL, Qt, OpenCV GUI paths, simulator viewers, and plotting backends that need native display libraries.

`x11-gl` provides Nix-managed userspace graphics/runtime libraries and points EGL at the matching Nix Mesa GLVND vendor file by default. This keeps Wayland/EGL clients from accidentally mixing Nix `libEGL.so.1` with a distro or container `/usr/share/glvnd` vendor file. Users may still override `__EGL_VENDOR_LIBRARY_FILENAMES` when a host-specific EGL vendor is required.

`qt6` provides Qt 6 runtime and development packages. It exposes Qt through standard `CMAKE_PREFIX_PATH` and `QT_PLUGIN_PATH` so ordinary CMake projects can use `find_package(Qt6 ...)` without robo-specific variables.

`x11-gl` does not scan host-specific NVIDIA driver directories or set host Vulkan paths. If a host driver path is missing, `robo doctor` reports the observed fact and the user-owned environment fix instead of expanding generated shell path guesses.
