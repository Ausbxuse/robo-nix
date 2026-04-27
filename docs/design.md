# Product Design

This document captures the intended product shape for `robo-nix` and the `robo` CLI.

## Positioning

`robo-nix` is a robot-learning environment toolkit powered by Nix and uv. Its intended user-facing CLI is `robo`.

It is not a Nix teaching project. It is not a Python package manager. It is not a central registry of every robot-learning stack.

The user-facing promise is:

> Keep your normal Python workflow. Let robo set up and diagnose the native robotics runtime.

The final goal is to fill the gap between `pyproject.toml` and the machine. uv can lock Python packages, but it cannot fully describe host libraries such as GL, FFmpeg, CUDA drivers, ROS tooling, or simulator runtimes. `robo-nix` should make that missing layer reproducible and debuggable.

## Target User

The primary user is a robot-learning researcher or engineer who currently reaches for Conda, Docker, shell scripts, and long setup READMEs.

They likely care about:

- training and evaluation code
- PyTorch/JAX/vision/simulation packages
- ROS, MuJoCo, Isaac, CUDA, OpenGL, FFmpeg, and native extensions
- fresh workstation setup
- reproducible collaboration across lab machines
- avoiding container friction for hardware, GUI, and iterative development

They likely do not want to understand:

- flakes
- derivations
- overlays
- Nixpkgs Python package sets
- why a missing `libGL.so.1` appears as a Python import failure

## Core Workflow

The intended beginner workflow is:

```bash
robo init robot-learning
cd robot-learning
robo doctor
robo sync
robo run pytest tests
robo develop
```

Command ownership:

- `robo init`: creates the project scaffold and generated Nix plumbing
- `robo doctor`: diagnoses host, runtime, uv, and workspace problems
- `robo sync`: bootstraps the project, then runs the uv sync path
- `robo develop`: bootstraps the project, then enters the Nix-backed runtime shell
- `robo run`: bootstraps the project, then runs `uv run` inside the prepared environment

In the current alpha, `robo` is exposed as a Nix app:

```bash
nix run github:ausbxuse/robo-nix#robo -- init .
```

After initialization, the direct backend path remains:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
nix develop
uv sync
```

The normal user path should stay on `robo`:

```bash
robo doctor
robo sync --group dev
robo run pytest tests
```

## File Ownership

The project should minimize the user-facing footprint of `flake.nix`.

Normal user-facing files:

```text
pyproject.toml
uv.lock
.python-version
src/
```

Runtime manifest:

```text
robo.nix
```

Generated plumbing:

```text
flake.nix
flake.lock
```

The CLI should treat `flake.nix` as managed plumbing. Beginner docs should not ask users to edit it.

## Python Boundary

uv owns Python:

- Python version
- `.python-version`
- virtual environment
- Python dependencies
- `uv.lock`

Nix owns runtime:

- `uv` itself
- compilers and build tools
- C/C++ runtimes
- OpenGL/Vulkan/X11/Wayland libraries
- FFmpeg and media libraries
- CUDA host/runtime expectations
- ROS and simulator tooling
- shell environment

This is the central design decision. Do not add Nix-managed Python as a first-class mode without strong real-world evidence.

## Non-Nix User Boundary

The beginner path must not require users to understand flakes, overlays, derivations, or package sets.

`robo` should:

- generate flake plumbing automatically
- probe `pyproject.toml` and common workspace layout before asking the user
- explain when Nix itself is missing or misconfigured
- point users to package search only when they need to add a native runtime component
- keep error messages in robotics/Python language first, Nix language second
- make generated files inspectable so interested users can gradually learn Nix

The product has no value if users must become Nix users before their robot-learning repo runs.

## Why Not `robo.toml`

Do not add `robo.toml` in the alpha design.

The project already has clear files of record:

- `pyproject.toml`: Python project metadata and dependencies
- `.python-version`: uv Python version
- `uv.lock`: Python dependency lock
- `robo.nix`: runtime components and platform policy
- `flake.nix`: generated Nix entrypoint

A `robo.toml` would be justified only if the CLI later needs configuration that does not belong in Python metadata or the runtime manifest. It must not duplicate Python dependencies, Python version, or runtime components.

## Why This Is Appealing

Senior robot-learning engineers will judge this project by whether it saves time on real machines.

The compelling cases are:

- `torch`, `opencv-python`, `pyav`, `mujoco`, simulator bindings, and native extensions need host libraries that Python tools do not model well
- Conda solves some Python setup but leaves many native runtime failures unexplained
- Docker gives isolation but makes robotics hardware, display, and iteration harder
- many research repos have fragile setup docs that work only on the original machine
- labs need repeatable onboarding for new workstations and collaborators

The high-value feature is not abstraction. It is actionable diagnosis:

```text
ERROR opencv import failed because libGL.so.1 is missing
Fix: add the x11-gl runtime component or run robo doctor for details
```

or:

```text
WARN NVIDIA driver was not detected
Isaac/CUDA workloads require a working host driver
```

If `doctor` can turn obscure runtime failures into concrete fixes, the project becomes useful.

## Maintainability Rules

Keep the core small:

- reusable runtime components only
- no central support for every robot
- no central Python package registry
- no preset matrix explosion
- no project-specific hacks unless they prove broadly reusable

Scale through downstream ownership:

- `robo init` for onboarding
- templates only after manual maintainer verdict
- `robo.nix` for project runtime policy
- `pyproject.toml` and `uv.lock` for Python policy
- project repos own robot/vendor specifics

## Product Standard

`robo-nix` should make a robotics environment easier to start, inspect, and fix than the equivalent Conda/Docker/README flow.

If users still need to understand Nix internals to get value, the product surface is wrong.
