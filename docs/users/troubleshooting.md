# Troubleshooting

The main debugging surface is `robo shell` or `robo run <command>`. Every
runtime attempt writes `.robo-nix/last-run.json` with redacted facts such as
dependency evidence, selected components, decision lines, and environment
variable names. When project setup fails, `robo` also writes
`.robo-nix/last-error.log` with pasteable context for an issue.

## Missing .python-version

`robo` does not choose a default Python version.

```bash
uv python pin 3.11
robo shell
```

## Existing non-robo flake

If a repository already has a non-robo `flake.nix`, `robo shell` refuses to
overwrite it. Decide whether this project should use that flake or a robo-nix
generated runtime before continuing.

## Missing Native Library

Errors such as these mean a runtime component is incomplete or missing:

```text
libstdc++.so.6: cannot open shared object file
libz.so.1: cannot open shared object file
Wayland: Failed to load libxkbcommon
```

For broad robotics packages, prefer fixing the component contract instead of
adding package-specific shell hacks. For example:

- `native-build` owns compiler runtime libraries.
- `linux-headers` owns Linux input headers.
- `desktop-gl` owns desktop graphics and GLFW windowing libraries.

If the missing file is a specific shared library, use `robo search` to find Nix
package candidates:

```bash
robo search libassimp.so
```

Then add the package that owns the failing library to `extraRuntimeLibraries` in
`robo.nix`.

## CUDA driver library not found

Errors such as these usually mean the host NVIDIA driver is not visible inside
the runtime:

```text
libcuda.so.1: cannot open shared object file
CUDA driver version is insufficient
CUDA_ERROR_UNKNOWN
```

For projects with CUDA wheel evidence in `uv.lock` or dependencies such as
`cuda-python`, `cupy-cuda12x`, `nvidia-curobo`, or `isaacsim`, `robo shell` and
`robo run` try to bridge host `libcuda.so.1` automatically. The probe checks
`ROBO_NIX_LIBCUDA_PATH`, `LD_LIBRARY_PATH`, `ldconfig -p`, and known host driver
locations.

Check the host directly:

```bash
nvidia-smi
ldconfig -p | grep libcuda.so.1
```

If the driver library is installed somewhere else, set:

```bash
export ROBO_NIX_LIBCUDA_PATH=/path/to/libcuda.so.1
```

To disable automatic host CUDA bridging:

```bash
export ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1
```

For verbose decision lines during setup:

```bash
ROBO_NIX_DEBUG=1 robo shell
```

## Existing robo.nix

After `robo.nix` exists, `robo shell` uses it as the canonical runtime manifest.
It will not re-infer dependencies or rewrite that file. Edit `robo.nix`
directly when a project needs another component.

## Runtime changed while shell is open

Active `robo shell` sessions check runtime input files at the next prompt. When
`flake.nix`, `flake.lock`, `.python-version`, `pyproject.toml`, `uv.lock`, or
`robo.nix` changes, `robo` re-evaluates the shell and exports refreshed
environment variables into the current shell.

This refresh does not run `uv sync` and does not rewrite `robo.nix`.

If two `robo` processes prepare the same project at once, robo-owned writes
under `.robo-nix/` use lock files. Set `ROBO_NIX_LOCK_TIMEOUT=<seconds>` to
change how long a process waits before reporting the held lock.

## Shell Selection

`robo shell` launches your normal interactive shell by default. If `$SHELL`
points at generic Nix Bash or plain `sh`, robo falls back to your login shell,
parent shell, or a shell from `PATH`.

To force a shell:

```bash
ROBO_NIX_SHELL=/run/current-system/sw/bin/zsh robo shell
```

The `[robo]` prompt prefix is injected through `.robo-nix/shell-startup/` and
does not edit your dotfiles.
