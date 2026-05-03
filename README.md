# robo-nix

`robo-nix` is a runtime bridge for uv-managed robotics projects.

- `uv` owns Python: `.python-version`, `.venv`, `pyproject.toml`, `uv.lock`.
- Nix owns native runtime: CUDA, graphics, ROS, simulators, compilers, shared libraries.
- `robo` owns workflow: `up`, `check`, `shell`, `status`, `run`.

The goal is simple: keep Python packaging normal while making the native robotics runtime reproducible, explicit, and easier to debug.

> [!WARNING]
> `robo-nix` is early beta software. Expect CLI wording, generated files, diagnostics, runtime coverage, and installer behavior to change while the project is validated against real robotics repositories. Review generated `robo.nix` and `flake.nix` before committing them, and pin versions for shared team workflows.

Read the documentation site at <https://ausbxuse.github.io/robo-nix/>.

## Quick Start

Install on a fresh Linux, macOS, or WSL host:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
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

Use `robo up --shell` when you want setup to drop directly into the runtime shell. Python package installation stays an explicit project step. `robo up` caches the realized shell environment in `.robo-nix/`, so later `robo shell` and `robo run ...` calls can start without repeating the full Nix shell evaluation unless runtime files change.

`robo up` creates missing runtime files when needed. `robo init` is still available when you only want to generate the project files. Existing `pyproject.toml` and `uv.lock` stay project-owned.

`robo status` prints a quick runtime health summary. `robo check` explains the next useful action when CUDA, graphics, ROS, native builds, or Python environment setup needs debugging. To leave a runtime shell, run `exit`; `robo deactivate` prints that clean exit path when users need a reminder.

For a Conda-like prompt prefix and in-place shell entry, install the optional shell hook:

```bash
eval "$(robo hook)"
robo shell
```

The hook supports bash and zsh in-place shell entry. Fish currently keeps the standard subprocess shell path.

## Product Boundary

`robo up` prepares the native runtime. It does not run `uv sync` or choose project-specific Python extras, dependency groups, indexes, or source pins. Those stay in `pyproject.toml`, `uv.lock`, and project docs.

`robo check cuda` and `robo check graphics --verbose` report observed host facts for CUDA and graphics. They do not scan arbitrary driver directories or guess host-specific library paths during shell setup.

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

`robo check --why` explains why components were selected. `robo contract --json` prints the resolved contract for tooling.

## Docs

The main documentation is published at <https://ausbxuse.github.io/robo-nix/>.

Build the VitePress documentation site locally:

```bash
nix build .#docs
```

Preview it locally:

```bash
nix run .#docs-serve
```

Useful entry points:

- [User guide](https://ausbxuse.github.io/robo-nix/users/getting-started)
- [Why robo-nix](https://ausbxuse.github.io/robo-nix/blog)
- [Diagnostics](https://ausbxuse.github.io/robo-nix/users/diagnostics)
- [Developer overview](https://ausbxuse.github.io/robo-nix/developers/overview)

## License

`robo-nix` is licensed under GPL-3.0-or-later. See [LICENSE](./LICENSE).
