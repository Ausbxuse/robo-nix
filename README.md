<div align="center">

# robo-nix

Native runtime environments for uv-based robotics projects.

<a href="https://ausbxuse.github.io/robo-nix/"><img alt="Docs" src="https://img.shields.io/badge/docs-online-6fb0f4?style=for-the-badge&labelColor=2c3144&color=6fb0f4"></a>
<a href="https://github.com/ausbxuse/robo-nix/releases"><img alt="Version" src="https://img.shields.io/github/v/tag/ausbxuse/robo-nix?sort=semver&style=for-the-badge&label=version&labelColor=2c3144&color=62bcc6"></a>
<a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-GPL%20v3.0-5d6784?style=for-the-badge&labelColor=2c3144&color=5d6784"></a>

</div>

`robo-nix` gives uv-managed robotics projects the native runtime layer Python packaging does not own.

Use `uv` for Python. Use `robo` for the runtime around it.

Example: instead of asking every Ubuntu user to install the right CUDA, OpenGL, FFmpeg, compiler, and simulator dependencies by hand, a repo can keep PyTorch, MuJoCo, and project code in `uv.lock` while `robo` prepares the native runtime. New contributors enter it with `robo up --shell`, then run the project's normal `uv sync`.

## Quick Start

Install `robo` on Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
```

The installer reuses what is already on your machine when it can. On a fresh machine, it installs the pieces `robo` needs, then adds the `robo` command.

Create a project and enter its runtime shell:

```bash
# Create a project with generated runtime files.
robo up robot-learning --yes
cd robot-learning

# Prepare the runtime and enter its shell.
robo up --shell
```

Then install Python packages with `uv`:

```bash
# Sync project-owned Python dependencies.
uv sync
```

After setup, use `robo run` for one-off commands or `robo shell` when you want to stay inside the runtime:

```bash
# Run one command inside the prepared runtime.
robo run python your_script.py

# Stay inside the runtime for interactive work.
robo shell

# Diagnose setup, Python, CUDA, graphics, and native build issues.
robo check
```

## Existing Projects

For a repository that already has Python project files, run:

```bash
cd existing-project

# Prepare the runtime and enter its shell.
robo up --shell

# Sync project-owned Python dependencies.
uv sync

python -m pytest
```

`uv sync` is separate on purpose: each project controls its own dependency groups, extras, private indexes, and editable sources.

## How It Works

Robotics setup usually fails where Python wheels meet host runtime requirements: graphics libraries, CUDA drivers, FFmpeg, compilers, ROS, and simulator tooling.

`robo-nix` keeps Python in `uv.lock`, generates reviewable Nix runtime files for the native layer, then runs commands through that prepared environment. Diagnostics report which layer failed: Python, Nix runtime, host driver, graphics, CUDA, or native builds.

For the deeper design, read the [developer architecture](https://ausbxuse.github.io/robo-nix/developers/architecture) and [runtime capability model](https://ausbxuse.github.io/robo-nix/developers/runtime-capability-model).

## Status

> [!WARNING]
> `robo-nix` is early beta software. Expect CLI wording, generated runtime files, diagnostics, runtime coverage, and installer behavior to change while the project is validated against real robotics repositories.
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

| Project                                                          | Relationship                                                                   |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [Nix](https://nixos.org/)                                        | Reproducible native runtime dependencies and isolated project environments.    |
| [uv](https://github.com/astral-sh/uv)                            | Python packages, virtual environments, lockfiles, and interpreter workflow.    |
| [nixpkgs-python](https://github.com/cachix/nixpkgs-python)       | Cached CPython coverage for uv-managed projects.                               |
| [nix-ros-overlay](https://github.com/lopsided98/nix-ros-overlay) | ROS package coverage for ROS-facing runtime components.                        |
| [nixGL](https://github.com/nix-community/nixGL)                  | Reference point for host graphics driver bridging.                             |
| [uv2nix](https://github.com/pyproject-nix/uv2nix)                | Nix-native Python packaging for projects that want Nix to own Python packages. |

## TODO

- [ ] Cache well-known Python versions and the `robo-nix` binary for faster first setup.

## License

`robo-nix` is licensed under GPL-3.0-or-later. See [LICENSE](./LICENSE).
