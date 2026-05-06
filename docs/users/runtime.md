# Runtime Support

`robo` prepares the runtime environment around Python: native libraries, compilers, CUDA and graphics support, ROS tooling, simulator dependencies, FFmpeg, and shell variables.

Python packages stay in `uv.lock`. Runtime pieces come from Nix and host diagnostics.

## What Belongs Where

- `uv` owns Python packages, CUDA wheels, virtual environments, dependency groups, package indexes, editable sources, and `uv.lock`.
- Nix owns native libraries, compilers, build tools, CUDA toolkit pieces when selected, graphics/media libraries, ROS tooling, and simulator support.
- The host owns hardware, GPU kernel drivers, display server policy, device access, containers, and vendor SDKs installed outside the project.

Each project gets its own pinned runtime, so native tool versions can differ across repositories without relying on global Ubuntu packages.

## CUDA

Python-only CUDA wheels usually need a compatible host driver but not the CUDA toolkit:

```text
torch + cu128 wheel -> host driver compatible with CUDA 12.8
```

Native CUDA extension builds need the toolkit component:

```text
CUDAExtension, .cu files, custom kernels -> cuda-toolkit
```

Run:

```bash
robo check cuda
```

Nix does not vendor the proprietary NVIDIA kernel driver. uv wheels do not install it. `robo` reports that boundary clearly.

For projects that declare CUDA wheels or Isaac Sim, `robo up` probes the host for `libcuda.so.1`. When it finds a confident provider, `robo run` and `robo shell` add that driver directory to the runtime automatically.

Override the detected path with:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

Disable host CUDA driver auto-bridging with:

```bash
export ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1
```

## Graphics

Graphics support is selected through runtime components such as `x11-gl` and `qt6`.

Use it for:

- MuJoCo and simulator viewers
- desktop OpenGL
- GLFW and EGL
- Qt applications and Qt-based build dependencies
- OpenCV GUI paths
- plotting backends that need native display libraries

`x11-gl` provides Nix-managed userspace graphics libraries and points EGL at a matching Nix Mesa GLVND vendor file by default.

On NVIDIA systems, desktop OpenGL viewers often need the host NVIDIA GLVND provider rather than Nix Mesa. For runtimes with `x11-gl`, `robo` may bridge detected host NVIDIA EGL/Vulkan manifests and a small project-local vendor library directory.

This is not an offload launcher. PRIME/offload remains host policy. Use the host's normal launch policy, such as `nvidia-offload`, when the machine needs it.

Disable host graphics auto-bridging with:

```bash
export ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO=1
```

Debug with:

```bash
robo check graphics --verbose
```

Known limits:

- AMD and Intel hosts currently rely on the Nix Mesa path.
- Wayland issues are diagnosed, not automatically repaired.
- Headless and remote rendering modes such as EGL-only rendering, OSMesa, VirtualGL, and container display forwarding still need explicit project or host setup.
- There is no nixGL-style one-command wrapper yet.

## ROS

Current ROS support is intentionally narrow:

- `ros2-jazzy` provides a ROS 2 Jazzy underlay, colcon tooling, `vcs`, CycloneDDS defaults, and the ROS setup environment.
- `ros-workspace` expects a workspace at `ros_ws/src` and exposes `ROBO_NIX_ROS_WS`.
- The `ros2-workspace` profile combines the base runtime, uv-managed Python, native build tools, `ros2-jazzy`, and `ros-workspace`.
- Supported systems for ROS 2 Jazzy are Linux only: `x86_64-linux` and `aarch64-linux`.

This is runtime infrastructure. It is not a ROS launcher, rosdep replacement, package registry, networking policy layer, or robot-specific bringup system.

The downstream project still owns:

- ROS packages and source repositories
- `rosdep` policy and package installation choices
- launch files
- robot-specific scripts
- ROS_DOMAIN_ID or ROS_LOCALHOST_ONLY choices when the defaults are wrong
- simulator, hardware, and networking orchestration

## Native Builds

Editable packages that build C/C++ or CUDA extensions still belong to the Python layer, but they often need native tools from the runtime.

The intended setup is:

- uv installs Python build helpers declared by the project.
- Nix provides compilers, CMake, pkg-config, headers, and native shared libraries.
- `robo shell` exposes CMake package prefixes from `.venv` packages when they provide `share/cmake`.

If CMake cannot find `pybind11`, `nanobind`, or another Python-owned helper, declare the helper package in the correct project dependency group.
