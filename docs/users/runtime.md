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
    <p>C/C++ build tools plus native runtime libraries such as libstdc++, zlib, and legacy libcrypt.</p>
  </div>
  <div>
    <h3>linux-headers</h3>
    <p>Linux input/kernel headers for native packages such as evdev.</p>
  </div>
  <div>
    <h3>desktop-gl</h3>
    <p>OpenGL, EGL, GLVND, Vulkan loader, Wayland, X11, GLFW windowing, GLU, and legacy Xt client libraries.</p>
  </div>
  <div>
    <h3>qt6</h3>
    <p>Qt6 CMake packages, tools such as qtpaths6, plugins, and runtime libraries for Qt services and viewers.</p>
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
  defaultProfile = "default";

  profiles = {
    default = {
      components = [
        "python-uv"
        "native-build"
        "linux-headers"
      ];

      pythonExtras = [];
      pythonGroups = [];

      extraPackages = pkgs: [
      ];

      extraRuntimeLibraries = pkgs: [
      ];
    };
  };
}
```

## Runtime profiles

Use profiles when one workspace contains multiple deployable runtime surfaces.
Each profile is a complete runtime manifest inside the same `robo.nix`:

```nix
{
  defaultProfile = "workstation";

  profiles = {
    workstation = {
      components = [ "python-uv" "native-build" "linux-headers" "desktop-gl" ];
      pythonExtras = [ "workstation" ];
      pythonGroups = [ "dev" ];
      hostGraphics = "auto";
    };

    tianji-driver = {
      pythonVersion = "3.10";
      components = [ "python-uv" "native-build" "linux-headers" ];
      pythonExtras = [ "tianji-driver" ];
      pythonGroups = [];
      hostGraphics = null;
    };
  };
}
```

`robo shell` uses `defaultProfile`. Select another runtime profile explicitly:

```bash
robo shell --profile tianji-driver
robo run --profile tianji-driver -- python -m dexmate.driver
robo refresh --profile tianji-driver
```

For profile-based manifests, robo sets `UV_PROJECT_ENVIRONMENT` to
`.robo-nix/venvs/<profile>/`. That keeps installed Python environments
decoupled, so syncing the driver profile does not overwrite workstation
packages.

Profiles inherit the workspace `.python-version` by default. Set
`pythonVersion = "3.10";` on a profile when a vendor tool needs a different
CPython from nixpkgs-python without moving the whole workspace to that version.

`pythonExtras` and `pythonGroups` are uv sync policy. Robo does not resolve
Python packages or edit `uv.lock`; it exports defaults through the uv wrapper so
plain `uv sync --locked` inside a runtime shell uses the selected profile's
extras and groups. Passing explicit uv flags such as `--extra`, `--group`, or
`--no-default-groups` overrides those defaults for that command.

Use `robo shell --profile <name> --sync` to run `uv sync --locked` with those
profile defaults before the interactive shell opens. `robo run --profile <name>
--sync -- <command>` does the same sync before launching one command.

The examples below show profile bodies. Put them under `profiles.<name>` in
`robo.nix`.

Project hooks may be plain strings when they only use the prepared runtime
environment:

```nix
shellHook = ''
  export ROBOT_CONFIG="$PWD/config/robot.yaml"
'';
```

Use `pkgs: string` when a hook needs a stable path from robo's pinned nixpkgs
set:

```nix
shellHook = pkgs: ''
  export ZLIB_ROOT="${pkgs.zlib}"
'';
```

Both forms run after robo prepares the selected profile's runtime libraries and
host graphics environment.

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
X11, Vulkan loader, GLVND, EGL, `libxkbcommon`, GLU, and legacy Xt client
libraries used by larger simulator stacks. It is application/runtime support,
not a GPU driver selector.

## Example: Qt service or viewer

Use `qt6` when a vendor service or local CMake project needs Qt6 packages such
as `Qt6::Core`, `Qt6::Network`, or `Qt6::Core5Compat`:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "desktop-gl"
    "qt6"
  ];
}
```

Host graphics wrapper selection is separate from `desktop-gl`. By default,
`hostGraphics = "auto";` uses `/run/opengl-driver` on NixOS hosts and the
generic robo-provided nixGL wrapper on other Linux hosts. If a simulator must
use the NVIDIA nixGL wrapper, set:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "desktop-gl"
    "cuda-toolkit"
  ];

  hostGraphics = "nixgl-nvidia";
}
```

Leave `hostGraphics = null;` when the project should not import a host graphics
wrapper. With `desktop-gl`, the Nix-managed client libraries still apply; they
do not select a GPU driver.

If a project already works correctly under a generic nixGL wrapper, set
`hostGraphics = "nixgl";` explicitly. In that mode, `robo` keeps using the
Nix-managed Python and runtime libraries from the project shell, then imports
only graphics-related variables from the selected nixGL wrapper. `robo-nix`
provides nixGL through its own flake inputs, so users do not need to install
nixGL in their profile for normal use. Use `hostGraphics = "nixgl-nvidia";`
only when the project must use the NVIDIA nixGL wrapper and should fail rather
than falling back to a Mesa wrapper. `robo` detects the host NVIDIA driver
version with `nvidia-smi` or `/proc/driver/nvidia/version`; set
`ROBO_NIX_NVIDIA_VERSION` only when those host probes are unavailable.

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

If a CUDA extension build needs GPU architecture hints, set
`cudaArchitectures`. This is for build systems that compile `.cu` kernels, not
for ordinary CUDA wheel runtime use:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "cuda-toolkit"
  ];

  cudaArchitectures = [ "8.9" ];
}
```

Use multiple values when building for more than one target GPU:

```nix
{
  cudaArchitectures = [ "8.6" "8.9" ];
}
```

For local workstation builds, `auto` best-effort detects NVIDIA GPUs with
`nvidia-smi`. If detection is unavailable, robo leaves the architecture hints
unset instead of failing shell startup:

```nix
{
  cudaArchitectures = "auto";
}
```

`cudaArchitectures` exports `TORCH_CUDA_ARCH_LIST`,
`CMAKE_CUDA_ARCHITECTURES`, and `CUDAARCHS`. It does not choose or validate
Python package versions, solve CUDA wheel compatibility, or replace the host
NVIDIA driver.

When a project appears to need host `libcuda.so.1`, `robo shell` and
`robo run` try to bridge a visible host driver library automatically. The probe
checks `ROBO_NIX_LIBCUDA_PATH`, `LD_LIBRARY_PATH`, `ldconfig -p`, and known
host driver locations used by common Linux and NixOS driver installs.

Inside `robo shell`, `UV_PYTHON` points at the Nix-managed CPython so uv creates
project environments from the runtime interpreter. For ad hoc installs,
`uv pip install ...` targets `$UV_PROJECT_ENVIRONMENT/bin/python` automatically
when that environment exists, unless you pass an explicit target such as
`--python`, `--active`, `--system`, `--target`, or `--prefix`.

You do not need to run `source .venv/bin/activate` inside `robo shell`. If a
copied setup command does activate the venv anyway, the virtualenv marker may
appear normally and the `[robo]` prompt marker stays single.

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
| `ROBO_NIX_NIXGL` | Override the nixGL wrapper path selected by `hostGraphics = "auto";`, `"nixgl";`, or `"nixgl-nvidia";`. |
| `ROBO_NIX_NVIDIA_VERSION` | Override the detected host NVIDIA driver version used by `hostGraphics = "nixgl-nvidia";`. |
| `ROBO_NIX_LOCK_TIMEOUT` | Seconds to wait for robo-owned `.robo-nix/*.lock` files. |
| `ROBO_NIX_DEFAULT_SOURCE_URL` | Override the generated flake input URL, commonly for local source testing. |

When `cudaArchitectures` is set, robo exports
`TORCH_CUDA_ARCH_LIST`, `CMAKE_CUDA_ARCHITECTURES`, and `CUDAARCHS` as build
hints for CUDA extension toolchains.

When `native-build` is selected, the shell also exports `ROBO_NIX_LIBC_DEV` as
the active compiler libc development prefix for build scripts that need to
inspect it.

Values that affect runtime construction, such as CUDA driver/toolkit paths, are
part of the active shell freshness key. Existing `robo shell` sessions refresh
at the next prompt when those inputs change. The key also follows common local
`.nix` imports from `robo.nix` and the project flake, so splitting runtime
libraries or component lists into helper Nix files keeps refresh behavior
truthful. When Nix reports evaluated local Nix files during a successful setup,
`robo` records those safe relative paths under `.robo-nix/` and folds them into
later refresh/cache keys.

After a successful setup, `robo` caches the captured runtime shell environment
by that same key. It registers the referenced Nix store paths as profile-owned
indirect GC roots under `.robo-nix/`, so later `robo shell` and `robo run`
attempts can reuse the local runtime without a network connection.

When runtime inputs change, robo still tries to evaluate the requested runtime
first. If that setup fails—for example, because a required remote input is
unreachable—it replays the Nix failure, clearly reports that it is using the
last working runtime environment, and launches it as long as every referenced
store path is present. The old environment is not re-keyed as current, so the
new input changes are never presented as applied.

This fallback requires at least one prior successful setup. `robo refresh` and
`robo update` intentionally clear runtime cache state and release its GC roots,
and `--sync` can still fail offline when uv needs package artifacts that are not
already cached.

The key uses semantic content for parseable `pyproject.toml`, `uv.lock`, and
`flake.lock`, so formatting, comments, and mapping order alone do not rerun Nix.
Comment-only changes in ordinary Nix runtime files are also ignored. Invalid or
unrecognized TOML/JSON and Nix source that cannot be scanned safely remain
byte-sensitive so cache reuse cannot hide a project error.

During runtime setup, `robo` passes its public binary cache settings to Nix
directly. That lets `robo shell` and `robo run` use the robo-nix caches even when
the host system substituter list does not include them. The host Nix daemon's
normal trust policy still applies. `robo` also prefetches cacheable runtime
inputs before entering `nix develop`, with local builds disabled for that
prefetch step.

Run `robo refresh` when you want to clear robo-owned local runtime state under
`.robo-nix/`. Inside an active `robo shell`, this requests a refresh through the
prompt hook; the shell updates at the next prompt. Outside a shell, the next
`robo shell` or `robo run` rebuilds the runtime cache.
