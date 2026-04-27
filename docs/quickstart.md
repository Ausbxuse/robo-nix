# Quickstart

This is the shortest current path to getting value from `robo-nix`.

For a more guided introduction, start with [Beginner Guide](./beginner-guide.md).

The intended user-facing CLI is `robo`:

```bash
robo init robot-learning
cd robot-learning
robo doctor
robo doctor --deep
robo sync
robo run pytest tests
robo develop
```

The current alpha exposes `robo` as a Nix app:

```bash
nix run github:ausbxuse/robo-nix#robo -- init .
```

`robo init` probes `pyproject.toml` by default. If it sees packages such as `mujoco`, `opencv-python`, `av`, `lerobot`, or Qt bindings, it adds the reusable native runtime components those packages usually need.

## What `robo-nix` Is

`robo-nix` currently publishes flake outputs for:

- reusable robotics-oriented components
- example preset environments
- a downstream constructor API: `robo-nix.lib.mkProjectFlake`
- a project initializer for generated downstream plumbing

The intended model is:

1. `robo-nix` owns reusable building blocks.
2. each downstream project declares its own composition locally
3. `robo-nix` generates `apps`, `devShells`, `packages`, and `checks`

## Initialize An Existing Project

Start from the project checkout. Assume there is no flake yet:

```bash
cd ~/src/dev/dexmate-teleop-develop
nix run github:ausbxuse/robo-nix#robo -- init .
```

That command:

- writes `flake.nix` as generated plumbing
- writes `robo.nix` as the runtime manifest
- writes `.python-version`
- creates `pyproject.toml` only if the project does not already have one
- keeps `uv.lock` and Python dependencies project-owned
- probes `pyproject.toml` and common workspace paths for runtime needs
- lets project-owned bootstrap scripts stay in the downstream repo

Generated projects point at the `robo-nix` source shipped with the CLI package by default. Use `--robo-nix-url path:/path/to/robo-nix` only when testing local `robo-nix` changes.
If you edit that local `robo-nix` source, refresh the generated project's lock before debugging runtime behavior:

```bash
nix flake lock --update-input robo-nix
```

`robo doctor` is lightweight by default and checks project files before realizing the full runtime closure. Use `robo doctor --deep` when debugging native libraries, GUI backends, or synced Python packages. Deep mode may cause Nix to realize more of the runtime.

`robo doctor` also warns when a generated project points at a dirty local `robo-nix` checkout because the lock may still reference an older source snapshot.

Packaged installs include bash, zsh, and fish completions.
Normal `robo` commands keep Nix bootstrap chatter quiet; add `--debug` when you need to see the exact subprocess commands and raw output.

Explain why runtime entries are present:

```bash
robo doctor --why
```

Emit machine-readable runtime/provenance output:

```bash
robo doctor --why --json
robo contract --json
```

List the recommended profiles directly:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --list-profiles
```

You can also inspect the reusable catalog without writing anything:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --list-components
```

For a guided prompt, use:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --interactive
```

If you want an example-driven path instead of the general overview, start with one of these:

- [Simple Environment Example](./simple-environment.md)
- [ROS 2 Workspace Example](./ros2-workspace-example.md)
- [Isaac Sim + ROS 2 Example](./isaac-ros2-example.md)

## Use A Preset Directly

Without creating a new flake yet, you can enter a published shell:

```bash
nix develop github:ausbxuse/robo-nix#robot-learning
nix develop github:ausbxuse/robo-nix#isaac-ros2-learning
```

Or bootstrap using the app entrypoint:

```bash
ROBO_NIX_WORKSPACE=$PWD \
nix run github:ausbxuse/robo-nix#isaac-ros2-learning -- --dry-run
```

## Start A New Project

For a fresh project:

```bash
nix run github:ausbxuse/robo-nix#robo -- init robot-learning --profile minimal
cd robot-learning
```

Then:

```bash
nix run github:ausbxuse/robo-nix#robo -- doctor
nix run github:ausbxuse/robo-nix#robo -- doctor --deep
nix run github:ausbxuse/robo-nix#robo -- sync
nix run github:ausbxuse/robo-nix#robo -- run pytest tests
nix run github:ausbxuse/robo-nix#robo -- develop
```

`robo sync`, `robo run`, and `robo develop` run project bootstrap first, then enter the Nix-backed runtime. `robo run pytest tests` expands to `uv run pytest tests` inside that runtime. Use the lower-level generated app when you specifically want to inspect bootstrap behavior:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
nix run .#default
```

## GUI Plotting And Desktop Backends

`robo init` treats Qt Python bindings such as `pyqt6`, `pyqt5`, and `pyside6` as a GUI signal and selects both `qt6` and `x11-gl`.
Those components provide native Qt, X11, OpenGL, font, DBus, and XCB runtime libraries; uv still owns the Python binding packages.

Matplotlib does not always select an interactive backend by itself. If a script calls `plt.show()`, run it with an explicit GUI backend:

```bash
MPLBACKEND=QtAgg robo run python graph.py
```

Or set the backend before importing `pyplot`:

```python
import matplotlib
matplotlib.use("QtAgg")

import matplotlib.pyplot as plt
```

`robo doctor --deep` probes synced projects for common PyQt and Matplotlib QtAgg failures and prints the missing native-library error when the Python package is installed but the runtime surface is incomplete.

## Vendor Source Trees

Some robotics projects rely on SDKs, patched upstream projects, or private source trees under `third_party/`.

Run:

```bash
robo vendor
```

`robo vendor` detects curated local vendor modules, clones only modules that have an explicit public `sourceUrl`, and runs configured bootstrap scripts. If a module has `sourceUrl = null`, robo will not fetch it and will tell you where to place the checkout.

Focused commands:

```bash
robo vendor list
robo vendor add third_party/GMR
robo vendor doctor
robo vendor bootstrap
robo vendor export dexmate-gmr
```

See [Vendor Workflow](./vendor-workflow.md).

## Core Commands

Common downstream commands:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
nix develop
uv sync
nix flake check
```

Common maintainer commands in this repo:

```bash
bash tests/dev-check.sh
bash tests/full-check.sh
```

## Next Reading

- [Simple Environment Example](./simple-environment.md)
- [Beginner Guide](./beginner-guide.md)
- [Product Design](./design.md)
- [Runtime Inference](./runtime-inference.md)
- [Vendor Workflow](./vendor-workflow.md)
- [ROS 2 Workspace Example](./ros2-workspace-example.md)
- [Isaac Sim + ROS 2 Example](./isaac-ros2-example.md)
- [Downstream Projects](./downstream-projects.md)
- [Components](./components.md)
- [Testing](./testing.md)
