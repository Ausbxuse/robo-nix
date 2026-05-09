# Runtime Components

Runtime components are named pieces in `robo.nix`. They describe what Nix should
make visible inside `robo shell`.

<div class="runtime-grid">
  <div>
    <h3>python-uv</h3>
    <p>CPython from nixpkgs-python plus uv. This is always present in generated projects.</p>
  </div>
  <div>
    <h3>native-build</h3>
    <p>C/C++ build tools plus native runtime libraries such as libstdc++ and zlib.</p>
  </div>
  <div>
    <h3>linux-headers</h3>
    <p>Linux input/kernel headers for native packages such as evdev.</p>
  </div>
  <div>
    <h3>desktop-gl</h3>
    <p>OpenGL, EGL, GLVND, Vulkan loader, Wayland, X11, and GLFW windowing support.</p>
  </div>
  <div>
    <h3>cuda-toolkit</h3>
    <p>Nix-managed CUDA compiler, headers, and CUDA build/link surface.</p>
  </div>
</div>

## CUDA Boundary

`cuda-toolkit` does not provide the host NVIDIA driver. If a workload needs
`libcuda.so.1`, point robo-nix at the host-owned driver library explicitly:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
robo shell
```

You may also set `ROBO_NIX_LIBCUDA_PATH` to a directory containing
`libcuda.so.1`.

## Editing robo.nix

`robo shell` may generate `robo.nix` on first bootstrap. After that, the file is
user-managed:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "desktop-gl"
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];
}
```

When a Python extension reports a missing shared library, search for the Nix
package that provides it:

```bash
robo search libassimp.so
```

`robo search` only prints candidates and a snippet. You still choose the package
and edit `robo.nix` yourself.
