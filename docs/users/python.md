# Python Boundary

Python is managed by `uv`, not by Nix.

This is a product decision, not an implementation accident. Most robotics and robot-learning projects already have normal Python files:

- `.python-version`
- `pyproject.toml`
- `uv.lock`
- `.venv`

`robo-nix` keeps those files in the Python toolchain's ownership and uses Nix for the native runtime that Python packaging cannot describe well.

## What uv Owns

uv owns:

- Python version selection
- virtual environment creation
- Python package resolution
- Python package installation
- dependency groups and optional extras
- `uv.lock`
- editable Python sources
- package indexes and credentials

If `uv sync` fails because an editable source is missing, a group was not selected, a package index is unavailable, or a build backend rejects the project, that is a Python/project-layer failure. `robo` should pass through enough output to debug it, but it should not guess project policy.

## What Nix Owns

Nix owns:

- native shared libraries
- C and C++ compilers
- CMake, pkg-config, and native build tools
- CUDA toolkit build surface when selected
- graphics and media runtime libraries
- ROS and simulator tooling
- shell environment variables needed by those runtime components

`robo shell` points uv at a Nix-provided CPython executable for the requested Python major/minor version. That keeps uv-created virtualenvs aligned with the Nix libc and dynamic loader, which matters on older distro containers.

robo-nix uses `cachix/nixpkgs-python` for Python interpreter coverage, then falls back to nixpkgs when that input does not provide the requested version. This keeps older robotics-friendly Python versions such as 3.11 available even after they leave current nixpkgs package sets. Generated flakes include the `nixpkgs-python.cachix.org` binary cache so normal users can fetch those interpreters instead of compiling CPython locally.

## Recreate Old Virtualenvs

If `.venv` was created before entering `robo shell`, recreate it:

```bash
robo shell
uv venv --clear
uv sync
```

A host-created virtualenv can load Nix `libstdc++`, graphics libraries, or FFmpeg libraries with the host distro glibc. On older distributions this can fail with errors like `GLIBC_2.38 not found`.

## Native Extension Builds

Editable packages that build C/C++ or CUDA extensions still belong to the Python layer, but they often need native tools from the runtime.

The intended setup is:

- uv installs Python build helpers declared by the project.
- Nix provides compilers, CMake, pkg-config, headers, and native shared libraries.
- `robo shell` avoids forcing Nix-managed Python include/library flags into setuptools or PyTorch extension builds.
- `robo shell` exposes CMake package prefixes from `.venv` packages when they provide `share/cmake`.

If CMake cannot find `pybind11`, `nanobind`, or another Python-owned helper, the usual fix is project-owned: declare the helper package in the correct uv group and configure build isolation appropriately.

## Tool Shims in `.venv`

Some Python packages ship executable shims such as `cmake`, `ninja`, or `patchelf`.

Those shims are Python package contents. Project bootstrap scripts that compile C/C++ should usually use the runtime's native tools instead of `.venv/bin` shims, especially in older containers where wheel binaries can mix host glibc with Nix runtime libraries.
