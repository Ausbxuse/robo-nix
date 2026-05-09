# Getting Started

`robo-nix` prepares the native runtime layer for robot-learning projects. uv
still owns Python packages and virtualenv sync.

Install once:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/rewrite/scripts/install.sh | sh
```

Then start from a Python project directory:

```bash
uv python pin <version>
robo shell
uv sync
```

See [Install](./install.md) for local checkout and installer override details.

`robo shell` requires `.python-version`. It creates the Nix runtime files when
they are missing, then enters your default interactive shell with a `[robo]`
prompt prefix. Set `ROBO_NIX_SHELL` when you need to override shell selection.
If runtime inputs change while that shell is open, `robo` refreshes the shell
environment at the next prompt.

`robo` does not create `pyproject.toml`. Use uv or your project tooling for
Python package metadata.

After first bootstrap, edit `robo.nix` for project runtime choices such as native
build tools or desktop graphics support.

If an error names a missing shared library, search for Nix package candidates:

```bash
robo search libassimp.so
```

Common runtime components:

- `desktop-gl`: Nix-managed OpenGL/EGL/Vulkan loader and desktop graphics
  libraries for GUI and simulator workloads.
- `linux-headers`: Linux kernel headers for native input-device packages such
  as `evdev`.
- `cuda-toolkit`: Nix-managed CUDA build toolkit surface for native CUDA
  extensions. The NVIDIA driver and `libcuda.so.1` still come from the host.

If a CUDA workload needs the host driver inside the runtime, set
`ROBO_NIX_LIBCUDA_PATH` to a `libcuda.so.1` path or to a directory containing
that file before running `robo shell`.

Run a command inside the runtime with:

```bash
robo run <command> [args...]
```
