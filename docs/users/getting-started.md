# Getting Started

Start here if you want to install `robo`, prepare a project, and run normal Python commands inside the runtime.

`robo-nix` gives uv-managed robotics projects a reproducible native runtime without asking every user to learn Nix first.

::: warning Early beta
`robo-nix` is early beta software. CLI wording, generated files, diagnostics, runtime coverage, and installer behavior may change. Review generated `robo.nix` and `flake.nix` before committing them, and pin versions before depending on it for a shared team workflow.
:::

The model is simple:

- `uv` owns Python versions, virtual environments, Python packages, and `uv.lock`.
- Nix owns native libraries, CUDA and graphics runtime pieces, ROS and simulator tooling, compilers, and shell environment.
- `robo` owns the workflow, generated runtime files, command wrapping, and diagnostics.

## Install

On a fresh Linux, macOS, or WSL host:

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh
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

`robo up` does not run `uv sync`. Python package installation is project-owned because repositories may require specific dependency groups, optional extras, private indexes, editable source checkouts, or install modes.

Use separate steps when you want to inspect the runtime before installing Python packages:

```bash
robo up
robo status
robo shell
uv sync
```

## Daily Workflow

After the first successful setup, the normal commands are:

```bash
robo run python -m pytest
robo shell
robo check
robo status
```

`robo shell` and `robo run` reuse cached runtime exports from `.robo-nix/` when the runtime files still match. If `robo.nix`, `flake.nix`, `.python-version`, or the runtime lock changes, `robo` refreshes the cache.

## What to Read Next

- [Why robo-nix](../blog.md) explains the motivation.
- [User workflow](./workflow.md) explains `up`, `shell`, `run`, `check`, `diagnose`, and `status`.
- [Python boundary](./python.md) explains why uv owns Python and how to avoid ABI mixing.
- [Diagnostics](./diagnostics.md) explains how to classify failures.
- [CUDA](./cuda.md), [graphics](./graphics.md), and [ROS](./ros.md) explain runtime-specific expectations.
