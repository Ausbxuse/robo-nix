# robo-nix

`robo-nix` is a runtime bridge for uv-managed robotics projects.

- `uv` owns Python: `.python-version`, `.venv`, `pyproject.toml`, `uv.lock`.
- Nix owns native runtime: CUDA, graphics, ROS, simulators, compilers, shared libraries.
- `robo` owns workflow: `up`, `doctor`, `shell`, `status`, `run`.

The goal is simple: keep Python packaging normal while making the native robotics runtime reproducible, explicit, and easier to debug.

## Quick Start

Install on a fresh Linux, macOS, or WSL host:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/main/scripts/install.sh | sh
```

The installer uses an existing `nix` when available. If Nix is missing, it installs Determinate Nix, then installs `robo` into the user's Nix profile.

Start a project:

```bash
robo up robot-learning --yes
cd robot-learning
robo up --shell
uv sync
robo run python -m pytest
robo status
```

Use `robo up --shell` when you want setup to drop directly into the runtime shell. Add `--sync` when you also want `robo up` to run `uv sync`; otherwise Python package installation stays an explicit project step. `robo up` caches the realized shell environment in `.robo-nix/`, so later `robo shell` and `robo run ...` calls can start without repeating the full Nix shell evaluation unless runtime files change.

`robo up` creates missing runtime files when needed. `robo init` is still available when you only want to generate the project files. Existing `pyproject.toml` and `uv.lock` stay project-owned.

`robo status` shows whether the current shell is inside a runtime shell. To leave a runtime shell, run `exit`; `robo deactivate` prints that clean exit path when users need a reminder.

For a Conda-like prompt prefix and in-place shell entry, install the optional shell hook:

```bash
eval "$(robo hook)"
robo shell
```

The hook supports bash and zsh in-place shell entry. Fish currently keeps the standard subprocess shell path.

## Product Boundary

`robo up` prepares the native runtime. `robo up --sync` can run `uv sync`, but it does not choose project-specific Python extras, dependency groups, indexes, or source pins. Those stay in `pyproject.toml`, `uv.lock`, and project docs.

`robo doctor --deep` reports observed host facts for CUDA and graphics. It does not scan arbitrary driver directories or guess host-specific library paths during shell setup.

## Repository Shape

```text
crates/robo-cli/       Rust CLI
nix/modules/           runtime component implementations
nix/metadata/          component docs, starter profiles, and inference rules
nix/mk-flake.nix       flake generator
nix/repo-support.nix   repo checks and package wrappers
tests/fixtures/        downstream flake fixtures
docs/                  VitePress documentation source
```

## Runtime Model

`robo.nix` is the project runtime contract. It lists reusable native components such as `python-uv`, `native-build`, `media`, `x11-gl`, `cuda-toolkit`, `ros2-jazzy`, and `mujoco`.

`robo doctor --why` explains why components were selected. `robo contract --json` prints the resolved contract for tooling.

## Docs

Build the VitePress documentation site:

```bash
nix build .#docs
```

Preview it locally:

```bash
nix run .#docs-serve
```

- [User guide](./docs/users/getting-started.md)
- [Why robo-nix](./docs/blog.md)
- [User workflow](./docs/users/workflow.md)
- [Python boundary](./docs/users/python.md)
- [Diagnostics](./docs/users/diagnostics.md)
- [CUDA](./docs/users/cuda.md)
- [Graphics](./docs/users/graphics.md)
- [ROS](./docs/users/ros.md)
- [Developer overview](./docs/developers/overview.md)
- [Architecture](./docs/developers/architecture.md)
- [CLI UX contract](./docs/developers/cli-ux.md)
- [Runtime capability model](./docs/developers/runtime-capability-model.md)
- [Repository workflow](./docs/developers/repository.md)
- [Roadmap](./docs/developers/roadmap.md)

## License

`robo-nix` is licensed under GPL-3.0-or-later. See [LICENSE](./LICENSE).
