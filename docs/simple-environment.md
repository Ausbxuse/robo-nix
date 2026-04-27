# Simple Environment Example

This is the most basic useful `robo-nix` flow: a small Python-first robotics project with a reproducible shell, a generated flake, and no ROS, simulator, or vendor SDK layers.

If you are new to Nix, start here.

## What This Example Gives You

- uv-managed Python `3.11`
- `uv` for Python version, virtualenv, and package management
- native build tooling for packages with compiled extensions
- a generated `flake.nix` and `robo.nix`
- `doctor`, `dry-run`, and `nix develop` as the main workflow

The generated environment maps to the maintained `minimal` profile internally.

## Fastest Path

Create a new project directory and let `robo-nix` generate the local adapter files:

```bash
mkdir simple-robot-project
cd simple-robot-project
nix run github:ausbxuse/robo-nix#robo -- init . \
  --name simple-project \
  --profile minimal
```

That writes:

- `flake.nix`
- `robo.nix`

## Validate The Setup

Run the generated environment checks before you start coding:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
```

What these do:

- `--doctor` checks that the generated project shape is valid and tells you what to do next
- `--dry-run` validates the bootstrap path without mutating the workspace

If both pass, enter the shell:

```bash
nix develop
```

## Install Python Dependencies

Inside the shell, manage your project dependencies the same way you normally would with `uv`:

```bash
uv init --python 3.11
uv add numpy
uv run python -c "import numpy; print(numpy.__version__)"
```

`robo-nix` is handling the system-level environment here. Your project still owns its Python dependencies and source tree.

## What The Generated Manifest Looks Like

The generated `robo.nix` is intentionally small:

```nix
{
  envName = "simple-project";
  description = "Research environment";
  components = [
    "base"
    "python-uv"
    "native-build"
  ];
  pythonVersion = "3.11";
  supportedSystems = [
    "x86_64-linux"
  ];
  workspaceRoot = ".";
}
```

That is the intended long-term model:

- `robo-nix` owns reusable capabilities
- your project owns the final composition

## Interactive Version

If you prefer prompts instead of flags:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --interactive
```

Then answer:

- advanced/manual setup: `no`
- ROS 2 workspace: `no`
- project setup: `None`

That path also lands on the same minimal profile.

## When To Use This Example

Use the simple environment when:

- you want a clean Python robotics shell
- you do not need ROS yet
- you do not need MuJoCo, Isaac, CUDA, or vendor SDKs yet
- you want the easiest possible entrypoint into `robo-nix`

If your project grows later, edit `robo.nix` or rerun `robo init` with a richer profile.
