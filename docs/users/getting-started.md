# Getting Started

`robo-nix` gives uv-managed robotics projects a reproducible native runtime without asking every user to learn Nix first.

The model is simple:

- `uv` owns Python versions, virtual environments, Python packages, and `uv.lock`.
- Nix owns native libraries, CUDA and graphics runtime pieces, ROS and simulator tooling, compilers, and shell environment.
- `robo` owns the workflow, generated runtime files, command wrapping, and diagnostics.

## Install

On a fresh Linux, macOS, or WSL host:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/main/scripts/install.sh | sh
```

The installer uses an existing `nix` when it can. If Nix is missing, it installs Determinate Nix, then installs `robo` into the user's Nix profile.

## Start a Project

Create a new project and prepare its runtime:

```bash
robo up robot-learning --yes
cd robot-learning
robo up --shell
uv sync
```

For an existing Python project:

```bash
cd existing-project
robo up --shell
uv sync
```

`robo up` creates or updates the project runtime files:

- `robo.nix`
- `flake.nix`
- `.python-version`
- `.robo-nix/` cache files

It does not rewrite project dependency policy. Existing `pyproject.toml`, `uv.lock`, dependency groups, optional extras, package indexes, editable sources, and project bootstrap scripts remain project-owned.

## First Run Options

Use `--shell` when setup should drop directly into the runtime shell:

```bash
robo up --shell
```

Interactive `robo up` may ask whether to run `uv sync`. The default is conservative because Python package installation can build native extensions, download large wheels, or trigger project-specific dependency choices.

Automation should opt in explicitly:

```bash
robo up --yes --sync
```

Use separate steps when you want to inspect the runtime before installing Python packages:

```bash
robo up
robo doctor
robo shell
uv sync
```

## Daily Workflow

After the first successful setup, the normal commands are:

```bash
robo run python -m pytest
robo shell
robo doctor
robo status
```

`robo shell` and `robo run` reuse cached runtime exports from `.robo-nix/` when the runtime files still match. If `robo.nix`, `flake.nix`, `.python-version`, or the runtime lock changes, `robo` refreshes the cache.

## What to Read Next

- [Why robo-nix](../blog.md) explains the motivation.
- [User workflow](./workflow.md) explains `up`, `shell`, `run`, `doctor`, and `status`.
- [Python boundary](./python.md) explains why uv owns Python and how to avoid ABI mixing.
- [Diagnostics](./diagnostics.md) explains how to classify failures.
- [CUDA](./cuda.md), [graphics](./graphics.md), and [ROS](./ros.md) explain runtime-specific expectations.
