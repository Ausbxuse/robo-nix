<div align="center">

# robo-nix

Native runtime environments for uv-based robot-learning projects.

<a href="https://ausbxuse.github.io/robo-nix/"><img alt="Docs" src="https://img.shields.io/badge/docs-online-6fb0f4?style=for-the-badge&labelColor=2c3144&color=6fb0f4"></a>
<a href="https://github.com/ausbxuse/robo-nix/releases"><img alt="Version" src="https://img.shields.io/github/v/tag/ausbxuse/robo-nix?sort=semver&style=for-the-badge&label=version&labelColor=2c3144&color=62bcc6"></a>
<a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-GPL--3.0-5d6784?style=for-the-badge&labelColor=2c3144&color=5d6784"></a>

</div>

`robo-nix` gives uv-based robot-learning projects the Nix-managed Python interpreter and native runtime layer Python packaging does not own.

Use `uv` for the Python dependency workflow. Use `robo` for the Nix-managed interpreter and native runtime.

Example: instead of asking every Ubuntu user to install the right CUDA, OpenGL, FFmpeg, compiler, and simulator dependencies by hand, a repo can keep PyTorch, MuJoCo, and project code in `uv.lock` while `robo` prepares the native runtime on demand. Once a repo has robo runtime files, users enter it with `robo shell`, then run the project's normal `uv sync`.

## Quick Start

Install `robo` on Linux, macOS, or WSL:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
```

The installer reuses what is already on your machine when it can. On a fresh machine, it installs the pieces `robo` needs, then adds the `robo` command. Linux is the regularly validated path today; macOS and WSL support are intended but not covered by the same validation yet. By default it installs from the current `develop` branch commit, not from the latest release tag.

Create a project, enter its runtime shell, and sync Python packages:

```bash
# Create a project with generated runtime files.
robo init robot-learning
cd robot-learning

# Add your Python package metadata and dependencies to pyproject.toml.
$EDITOR pyproject.toml

# Enter the Nix-managed runtime shell. Native libraries and tools are built on demand.
robo shell

# Sync project-owned Python packages into .venv.
uv sync

# Run your project code inside the prepared runtime.
python train.py

# Leave the runtime shell when you are done.
exit
```

After first-time setup, choose one way to run project commands:

```bash
# For normal interactive work, enter the runtime shell.
robo shell

# Or, for one-off commands, run through the runtime without opening a shell.
robo run python train.py
```

When setup, Python, CUDA, graphics, or native builds fail, run diagnostics from the project directory:

```bash
robo check
```

## Existing Projects

For a repository that already has `robo.nix` and `flake.nix`, enter the runtime directly:

```bash
cd existing-project

# Enter the Nix-managed runtime shell. Native libraries and tools are built on demand.
robo shell

# Sync project-owned Python packages into .venv.
uv sync

# Run your project code inside the prepared runtime.
python train.py
```

For an existing Python repository without robo runtime files, `robo shell` asks before creating them:

```bash
cd existing-python-project
robo shell
uv sync
```

Run `robo init .` first when you want to review the generated files before entering the runtime.

`uv sync` is separate on purpose: each project controls its own dependency groups, extras, private indexes, and editable sources.

Use `robo build` only when you want to prebuild the runtime without entering a shell, such as in CI or before working offline:

```bash
robo build
```

## Tips & Tricks

- `uv` is available inside `robo shell`, so you do not have to install `uv` manually. Run `uv sync`, `uv add`, `uv lock`, and other project Python commands there so packages build and import against the robo runtime.
- Use `robo run <command>` for one-off commands from your normal shell, and `robo shell` when you want to run several commands interactively.
- If a `.venv` was created before entering `robo shell`, recreate it inside the runtime with `uv venv --python "$ROBO_NIX_PYTHON" --clear`, then run `uv sync`.
- If an import fails with a missing shared library such as `libassimp.so`, run `robo search libassimp.so` to find the Nix package to add to `extraRuntimeLibraries`.
- If native packages fail because a library, compiler, CUDA, graphics, ROS, or simulator dependency is missing, update `components` in `robo.nix`, then run `robo build` or re-enter `robo shell`.

## How It Works

Robot-learning setup usually fails where Python wheels meet host runtime requirements: graphics libraries, CUDA drivers, FFmpeg, compilers, ROS, and simulator tooling.

`robo-nix` keeps Python version and package policy in uv-managed files, generates reviewable Nix runtime files for the interpreter and native layer, then runs commands through that prepared environment. Diagnostics report which layer failed: Python, Nix runtime, host driver, graphics, CUDA, or native builds.

For the deeper design, read the [developer architecture](https://ausbxuse.github.io/robo-nix/developers/architecture) and [runtime capability model](https://ausbxuse.github.io/robo-nix/developers/runtime-capability-model).

## Status

> [!WARNING]
> `robo-nix` is early beta software. Expect CLI wording, generated runtime files, diagnostics, runtime coverage, and installer behavior to change while the project is validated against real robot-learning repositories.
>
> Contribution standards, including AI usage and disclosure, live in [Contributing](./CONTRIBUTING.md).

## Docs

Read the documentation site at <https://ausbxuse.github.io/robo-nix/>.

Useful entry points:

- [User guide](https://ausbxuse.github.io/robo-nix/users/)
- [Why robo-nix](https://ausbxuse.github.io/robo-nix/blog)
- [Troubleshooting](https://ausbxuse.github.io/robo-nix/users/troubleshooting)
- [Runtime support](https://ausbxuse.github.io/robo-nix/users/runtime)
- [Developer guide](https://ausbxuse.github.io/robo-nix/developers/)

Contributor setup, repository layout, and local documentation commands live in the [Developer guide](https://ausbxuse.github.io/robo-nix/developers/).

## Related Projects

`robo-nix` is built around these tools and ecosystems:

| Project                                                          | Relationship                                                                    |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [Nix](https://nixos.org/)                                        | Reproducible Python interpreter, native runtime dependencies, and environments. |
| [uv](https://github.com/astral-sh/uv)                            | Python version requests, packages, virtual environments, and lockfiles.         |
| [nixpkgs-python](https://github.com/cachix/nixpkgs-python)       | Cached CPython interpreter coverage for uv-managed projects.                    |
| [nix-ros-overlay](https://github.com/lopsided98/nix-ros-overlay) | ROS package coverage for ROS-facing runtime components.                         |
| [nixGL](https://github.com/nix-community/nixGL)                  | Reference point for host graphics driver bridging.                              |
| [uv2nix](https://github.com/pyproject-nix/uv2nix)                | Nix-native Python packaging for projects that want Nix to own Python packages.  |

## TODO

- [ ] Cache well-known Python versions and the `robo-nix` binary for faster first setup.
- [ ] Add `robo add <component...>` as a narrow helper for adding known runtime components to `robo.nix`

## License

`robo-nix` is licensed under GPL-3.0. See [LICENSE](./LICENSE).
