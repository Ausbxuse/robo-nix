# robo-nix

`robo-nix` is a small runtime bridge for uv-managed robotics projects.

- `uv` owns Python: `.python-version`, `.venv`, `pyproject.toml`, `uv.lock`.
- Nix owns native runtime: CUDA, graphics, ROS, simulators, compilers, shared libraries.
- `robo` owns workflow: `up`, `doctor`, `shell`, `status`, `run`.

The goal is that a robot-learning project can keep normal Python packaging while still getting the native libraries that Python metadata cannot express.

## Quick Start

Current alpha usage:

```bash
nix run github:ausbxuse/robo-nix#robo -- up --yes
nix run github:ausbxuse/robo-nix#robo -- run pytest tests
```

The intended installed flow is:

```bash
robo up robot-learning --yes
cd robot-learning
robo run pytest tests
robo status
```

`robo init` writes generated Nix plumbing plus a small `robo.nix`. Existing `pyproject.toml` and `uv.lock` stay project-owned.

`robo status` shows whether the current shell is inside a runtime shell. To leave a runtime shell, run `exit`; `robo deactivate` prints that clean exit path when users need a reminder.

For a Conda-like prompt prefix and in-place shell entry, install the optional shell hook:

```bash
eval "$(robo hook)"
robo shell
```

The hook supports bash and zsh in-place shell entry. Fish currently keeps the standard subprocess shell path.

## Product Boundary

`robo up` prepares the native runtime and can run `uv sync`, but it does not choose project-specific Python extras, dependency groups, indexes, or source pins. Those stay in `pyproject.toml`, `uv.lock`, and project docs.

`robo doctor --deep` reports observed host facts for CUDA and graphics. It does not scan arbitrary driver directories or guess host-specific library paths during shell setup.

## Repository Shape

```text
crates/robo-cli/       Rust CLI
nix/modules/           runtime component implementations
nix/metadata/          component docs, starter profiles, and inference rules
nix/mk-flake.nix       flake generator
nix/repo-support.nix   repo checks and package wrappers
tests/fixtures/        downstream flake fixtures
docs/                  short concept docs
```

## Runtime Model

`robo.nix` is the project runtime contract. It lists reusable native components such as `python-uv`, `native-build`, `media`, `x11-gl`, `cuda-toolkit`, `ros2-jazzy`, and `mujoco`.

`robo doctor --why` explains why components were selected. `robo contract --json` prints the resolved contract for tooling.

## Docs

- [Architecture](./docs/architecture.md)
- [CLI UX](./docs/cli-ux.md)
- [Python](./docs/python.md)
- [CUDA](./docs/cuda.md)
- [Graphics](./docs/graphics.md)
- [ROS](./docs/ros.md)
- [Diagnostics](./docs/diagnostics.md)
- [Roadmap](./docs/roadmap.md)
