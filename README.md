<div align="center">

# robo-nix

Robot-learning environments that keep Python normal.

<a href="https://ausbxuse.github.io/robo-nix/">
  <img alt="Docs" src="https://img.shields.io/badge/docs-online-6fb0f4?style=for-the-badge&labelColor=2c3144&color=6fb0f4">
</a>
<a href="https://ausbxuse.github.io/robo-nix/users/getting-started">
  <img alt="Get Started" src="https://img.shields.io/badge/get_started-user_guide-8ac48a?style=for-the-badge&labelColor=2c3144&color=8ac48a">
</a>
<a href="https://github.com/ausbxuse/robo-nix/releases">
  <img alt="Release" src="https://img.shields.io/github/v/release/ausbxuse/robo-nix?include_prereleases&display_name=tag&style=for-the-badge&label=release&labelColor=2c3144&color=62bcc6">
</a>
<a href="./LICENSE">
  <img alt="License" src="https://img.shields.io/badge/license-GPL%20v3.0-5d6784?style=for-the-badge&labelColor=2c3144&color=5d6784">
</a>

</div>

`robo-nix` is for robotics projects that already use Python, but still need the native runtime pieces Python packaging does not provide: CUDA, OpenGL, ROS tooling, simulators, compilers, FFmpeg, and other shared libraries.

Use `uv` for Python. Use `robo` for the runtime around it.

## Why

Robotics setup usually fails at the boundary between Python and the host system. A wheel imports fine on one machine and fails on another because `libGL.so.1`, CUDA, FFmpeg, a compiler, ROS, or a simulator dependency is missing.

`robo-nix` makes that boundary explicit. Python dependencies stay in `pyproject.toml` and `uv.lock`; runtime dependencies live in Nix-backed project files that `robo` can generate, enter, and diagnose.

> [!WARNING]
> `robo-nix` is early beta software. Expect CLI wording, generated runtime files, diagnostics, runtime coverage, and installer behavior to change while the project is validated against real robotics repositories. Review generated project files before committing them, and pin versions for shared team workflows.

## Tested Platforms

This table describes the checks run by CI and scheduled validation jobs. Container-based checks cover clean-image setup plus `robo init`, `robo check`, and `robo status`; they do not prove desktop graphics, host GPU drivers, hardware access, shell entry, or ROS networking on that distribution.

| Platform | CI targets | Install / Nix setup | `robo init/check/status` | `robo shell` | GPU / graphics | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Ubuntu Linux | runners: 22.04, 24.04; containers: 22.04, 24.04 | ✓ | ✓ | ✓ | ✗ | Runner jobs cover shell validation; containers cover clean-image setup. |
| Debian Linux | `debian:12`, `debian:13` | ✓ | ✓ | ✗ | ✗ | Clean container validation. |
| Fedora Linux | `fedora:latest` | ✓ | ✓ | ✗ | ✗ | Clean container validation. |
| Arch Linux | `archlinux:latest` | ✓ | ✓ | ✗ | ✗ | Clean container validation. |
| NixOS / Nix image | `nixos/nix:latest` | ✓ | ✓ | ✗ | ✗ | Starts from an image with Nix already available. |
| macOS | `macos-latest` | ✓ | partial | ✗ | ✗ | Evaluates library APIs and flake outputs; Linux runtime shells are not exercised. |
| NVIDIA Linux host | self-hosted runner with `linux`, `x64`, `gpu`, `nvidia` labels | ✓ | ✓ | ✓ | ✓ | Scheduled CUDA and GPU validation. |

## Key Features

| Feature | What it means |
| --- | --- |
| Normal Python workflow | Keep using `.python-version`, `.venv`, `pyproject.toml`, and `uv.lock`. |
| Robotics runtime setup | Prepare native libraries, CUDA and graphics pieces, ROS and simulator tooling, compilers, and shell environment. |
| Small command surface | Use `robo up`, `robo run`, `robo shell`, `robo check`, and `robo status` instead of copying long setup recipes. |
| Runtime diagnostics | Separate Python, runtime, host driver, graphics, CUDA, and native build failures. |
| Explicit project files | Generate reviewable runtime files instead of relying on hidden machine state. |
| Fast repeat runs | Reuse cached runtime exports when the project runtime has not changed. |

## How It Compares

`robo-nix` is focused on uv-managed robotics projects that need more than Python packages. It is not trying to replace every environment tool.

| Tool | Best fit | Where `robo-nix` is different |
| --- | --- | --- |
| `uv` | Fast Python package and virtual environment management. | `robo-nix` keeps `uv` in charge of Python and adds the native robotics runtime around it. |
| Poetry | Python packaging and publishing workflows. | `robo-nix` does not manage Python packaging policy; it prepares the runtime Python packages need. |
| Conda / Pixi | Teams that want one ecosystem to own Python and many native packages. | `robo-nix` keeps Python project files normal and focuses on robotics runtime setup and diagnostics. |
| Docker / dev containers | Image-based development, deployment, and isolated services. | `robo-nix` targets host-integrated robotics work where GPUs, simulator GUIs, ROS networking, and hardware access matter. |
| Handwritten setup docs | Small projects with simple dependencies. | `robo-nix` turns repeat setup assumptions into project files and reusable commands. |
| General dev-shell tools | Teams already comfortable maintaining low-level environment definitions. | `robo` provides a robotics-oriented workflow, generated project files, and plain-language checks for users who should not need to become environment experts. |

## Quick Start

Install `robo` on Linux, macOS, or WSL:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
```

The installer reuses what is already on your machine when it can. On a fresh machine, it installs the pieces `robo` needs, then adds the `robo` command.

### Core Commands

Learn these first. They are the day-to-day surface of `robo-nix`.

| Command              | Use it when you want to...                                     |
| -------------------- | -------------------------------------------------------------- |
| `robo up`            | Create or refresh the project runtime.                         |
| `robo up --shell`    | Create or refresh the runtime, then enter it immediately.      |
| `robo run <command>` | Run one command inside the prepared runtime.                   |
| `robo shell`         | Stay inside the runtime for interactive work.                  |
| `robo check`         | Debug setup, Python, CUDA, graphics, and native build issues.  |
| `robo status`        | See a quick health summary for the current project.            |

### New Project

Create a project and enter its runtime shell:

```bash
robo up robot-learning --yes
cd robot-learning
robo up --shell
```

Then install Python packages with `uv`:

```bash
uv sync
```

Run a quick validation check:

```bash
python -c "import sys; print(sys.executable)"
```

Leave the shell with `exit`. After that, use `robo run` for one-off commands or `robo shell` when you want to stay inside the runtime:

```bash
robo run python your_script.py
robo shell
robo check
```

### Existing Project

For a repository that already has Python project files, run:

```bash
cd existing-project
robo up --shell
uv sync
python -m pytest
```

`robo up --shell` prepares the runtime and drops you into it. `uv sync` is separate on purpose: each project controls its own dependency groups, extras, private indexes, and editable sources.

After setup, keep using `robo` as the entry point:

```bash
robo run python -m pytest
robo run python train.py
robo check
robo shell
robo status
```

### Optional Shell Hook

For a Conda-like prompt prefix and in-place shell entry, install the optional shell hook:

```bash
eval "$(robo hook)"
robo shell
```

The hook supports bash and zsh in-place shell entry. Fish currently keeps the standard subprocess shell path.

## How It Works

`robo-nix` keeps ownership clear:

- `uv` owns Python versions, virtual environments, Python packages, and `uv.lock`.
- The runtime layer owns native libraries, CUDA and graphics pieces, ROS and simulator tooling, compilers, and the shell environment.
- `robo` owns workflow, generated runtime files, command wrapping, and diagnostics.

`robo up` creates or updates the project runtime files, prepares the runtime, and caches shell exports in `.robo-nix/` so later `robo shell` and `robo run ...` commands can start faster.

Python package installation remains explicit. `robo` does not run `uv sync` for you, because dependency groups, extras, private indexes, editable sources, and install policy belong to each project.

## Documentation

Read the documentation site at <https://ausbxuse.github.io/robo-nix/>.

Useful entry points:

- [Get started](https://ausbxuse.github.io/robo-nix/users/getting-started)
- [Why robo-nix](https://ausbxuse.github.io/robo-nix/blog)
- [Diagnostics](https://ausbxuse.github.io/robo-nix/users/diagnostics)
- [Python boundary](https://ausbxuse.github.io/robo-nix/users/python)
- [Developer overview](https://ausbxuse.github.io/robo-nix/developers/overview)

Contributor setup, repository layout, and local documentation commands live in the [Developer overview](https://ausbxuse.github.io/robo-nix/developers/overview).

## License

`robo-nix` is licensed under GPL-3.0-or-later. See [LICENSE](./LICENSE).
