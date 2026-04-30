# Graphics

Graphics runtime libraries are selected through `x11-gl` and `qt6`.

Use these for desktop OpenGL, Qt, OpenCV GUI paths, simulator viewers, and plotting backends that need native display libraries.

`x11-gl` provides Nix-managed userspace graphics/runtime libraries. It does not scan host-specific NVIDIA driver directories or set host Vulkan/EGL vendor paths. If a host driver path is missing, `robo check` should report the observed fact and the user-owned environment fix instead of expanding generated shell path guesses.

TODO(robo): keep graphics inference data-driven in `nix/metadata/runtime-inference.nix`.
