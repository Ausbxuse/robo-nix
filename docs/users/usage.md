# Usage

Install `robo` on Linux, macOS, or WSL:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
```

The installer uses an existing `nix` when it can. If Nix is missing, it installs Determinate Nix, then installs `robo` into the user's Nix profile. By default it installs from the current `develop` branch commit, not from the latest release tag.

## New Project

Create a project, add Python metadata, and enter its runtime shell:

```bash
# Create a project with generated runtime files.
robo init robot-learning
cd robot-learning

# Add your Python package metadata and dependencies.
$EDITOR pyproject.toml

# Enter the runtime shell. The native runtime is built on demand.
robo shell

# Sync project-owned Python dependencies.
uv sync
```

## Existing Project

For a repository that already has `robo.nix` and `flake.nix`, enter its runtime shell from the repository root:

```bash
cd existing-project

# Enter the runtime shell. The runtime is built on demand.
robo shell

# Sync project-owned Python dependencies.
uv sync

python -m pytest
```

For an existing Python repository without robo runtime files, initialize it first:

```bash
cd existing-python-project
robo init .
robo shell
uv sync
```

`uv sync` is separate on purpose. Each project controls its own dependency groups, optional extras, private indexes, editable sources, and install policy.

## Daily Commands

Use `robo run` from the project directory when you want one command to run inside the project runtime:

```bash
robo run python -m pytest
robo run python train.py
```

Use `robo shell` from the project directory for interactive work:

```bash
robo shell
# work inside the runtime
exit
```

Use `robo check` from the project directory when the environment is not behaving:

```bash
robo check
robo check cuda
robo check graphics
robo check --deep
```

Use `robo status` from the project directory for a quick health summary:

```bash
robo status
```

Use `robo build` when you want to prebuild the runtime without entering a shell, such as in CI or before a long offline session:

```bash
robo build
```

## Generated Files

After `robo init`, a project usually contains:

```text
robo.nix          project runtime choices, such as CUDA, graphics, ROS, or native tools
flake.nix         generated Nix entry point used by robo
.python-version   Python version selected by uv
.robo-nix/        runtime cache and implementation details
```

Commit `robo.nix`, `flake.nix`, and `.python-version` when the team should share that runtime. Treat `.robo-nix/` as generated cache.

## Python Boundary

Python is split deliberately. `uv` owns the project Python workflow:

- Python version selection through `.python-version` or `uv` commands
- virtual environment creation
- Python package resolution and installation
- dependency groups and optional extras
- `uv.lock`
- editable Python sources
- package indexes and credentials

Nix owns the Python interpreter and the native/runtime layer around it:

- CPython interpreter provision for the selected version
- shared libraries
- C and C++ compilers
- CMake, pkg-config, and native build tools
- CUDA toolkit build surface when selected
- graphics and media runtime libraries
- ROS and simulator tooling
- shell environment variables for runtime components

If `.venv` was created before entering `robo shell`, recreate it:

```bash
robo shell
uv venv --python "$ROBO_NIX_PYTHON" --clear
uv sync
```

A host-created virtualenv can load Nix native libraries through the wrong host ABI, especially in older distro containers.
