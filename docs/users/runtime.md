# Runtime Examples

This is a user topic, not a separate product area. `robo.nix` is the file you
edit when a project needs more native tools or runtime libraries inside
`robo shell`.

## Components

<div class="runtime-grid">
  <div>
    <h3>python-uv</h3>
    <p>CPython from nixpkgs-python plus uv, including the CPython shared library path. This is always present in generated projects.</p>
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

## Example: native input package

Use this when a Python package such as `evdev` builds against Linux input
headers:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "linux-headers"
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];
}
```

## Example: simulator or desktop window

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

`desktop-gl` covers the common GLFW Linux windowing path, including Wayland,
X11, Vulkan loader, GLVND, EGL, and `libxkbcommon`.

Host GPU provider selection is separate from `desktop-gl`. If a simulator such
as Isaac Sim needs the host NVIDIA Vulkan/EGL/GLX provider, set the explicit
manifest policy:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "desktop-gl"
    "cuda-toolkit"
  ];

  hostGraphics = "nvidia";
}
```

Leave `hostGraphics = null;` when the host session should choose the graphics
provider. The generated `robo.nix` includes comments for the supported options.

## Example: CUDA extension build

Use `cuda-toolkit` when a Python package builds native CUDA extensions. The host
NVIDIA driver is still outside the Nix-managed toolkit:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "cuda-toolkit"
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];
}
```

When a project appears to need host `libcuda.so.1`, `robo shell` and
`robo run` try to bridge a visible host driver library automatically. The probe
checks `ROBO_NIX_LIBCUDA_PATH`, `LD_LIBRARY_PATH`, `ldconfig -p`, and the same
known host driver locations used by robo's CUDA bridge.

Inside `robo shell`, `UV_PYTHON` points at the Nix-managed CPython so uv creates
project environments from the runtime interpreter. For ad hoc installs,
`uv pip install ...` targets `$UV_PROJECT_ENVIRONMENT/bin/python` automatically
when that environment exists, unless you pass an explicit target such as
`--python`, `--active`, `--system`, `--target`, or `--prefix`.

You do not need to run `source .venv/bin/activate` inside `robo shell`. If a
copied setup command does activate the venv anyway, the runtime shell disables
virtualenv prompt rewrites so the `[robo]` prompt marker stays single.

Override the detected library explicitly when the driver lives elsewhere:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
robo shell
```

You may also set `ROBO_NIX_LIBCUDA_PATH` to a directory containing
`libcuda.so.1`. Disable automatic host CUDA bridging with:

```bash
export ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1
```

Useful host checks:

```bash
nvidia-smi
ldconfig -p | grep libcuda.so.1
```

Expected driver-boundary failures include `libcuda.so.1: cannot open shared
object file`, `CUDA driver version is insufficient`, and CUDA driver API errors
from packages such as Triton or CUDA Python. Nix can provide CUDA build tools,
but the NVIDIA kernel driver and `libcuda.so.1` still come from the host.

If the Nix CUDA toolkit root needs to come from a local driver/toolkit install,
set `ROBO_NIX_CUDA_ROOT=/path/to/cuda`. This changes the toolkit path exposed
by the `cuda-toolkit` component; it does not make `robo` own the host kernel
driver.

## Adding a missing shared library

When a Python extension reports a missing shared library, search for the Nix
package that provides it:

```bash
robo search libassimp.so
```

`robo search` only prints candidates and a snippet. You still choose the package
and edit `robo.nix` yourself:

```nix
{
  components = [
    "python-uv"
    "native-build"
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
    pkgs.assimp
  ];
}
```

## Environment variables

Public environment knobs are intentionally small:

| Variable | Purpose |
| --- | --- |
| `ROBO_NIX_SHELL` | Override the interactive shell launched by `robo shell`. |
| `ROBO_NIX_DEBUG` | Print debug lines and use plain progress rendering. |
| `ROBO_NIX_NO_SPINNER` | Disable spinner/progress tree rendering. |
| `ROBO_NIX_LIBCUDA_PATH` | Explicit host `libcuda.so.1` file or containing directory. |
| `ROBO_NIX_DISABLE_HOST_CUDA_AUTO` | Disable automatic host CUDA bridge probing. |
| `ROBO_NIX_CUDA_ROOT` | Override the CUDA toolkit root exported by `cuda-toolkit`. |
| `ROBO_NIX_NVIDIA_VK_ICD` | Override the Vulkan ICD path used by `hostGraphics = "nvidia";`. |
| `ROBO_NIX_NVIDIA_EGL_VENDOR` | Override the EGL vendor JSON path used by `hostGraphics = "nvidia";`. |
| `ROBO_NIX_LOCK_TIMEOUT` | Seconds to wait for robo-owned `.robo-nix/*.lock` files. |
| `ROBO_NIX_DEFAULT_SOURCE_URL` | Override the generated flake input URL for local development. |

When `native-build` is selected, the shell also exports `ROBO_NIX_LIBC_DEV` as
the active compiler libc development prefix for build scripts that need to
inspect it.

Values that affect runtime construction, such as CUDA driver/toolkit paths, are
part of the active shell freshness key. Existing `robo shell` sessions refresh
at the next prompt when those inputs change.

After a successful setup, `robo` caches the captured runtime shell environment by
that same key. Later `robo shell` and `robo run` attempts can reuse it instantly
as long as the referenced Nix store paths still exist.
