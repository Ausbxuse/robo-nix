# Beginner Guide

This page is for people who want to work on a robot-learning project, not learn Nix first.

## The Short Version

Inside a project checkout:

```bash
robo doctor
robo sync
robo run python your_script.py
robo develop
```

In the current alpha, if `robo` is not installed directly yet, run it through Nix:

```bash
nix run github:ausbxuse/robo-nix#robo -- doctor
```

## What Robo Owns

`robo` manages the native runtime around your Python project:

- OpenGL, X11, Qt, FFmpeg, MuJoCo, ROS, build tools, and similar native dependencies
- runtime diagnostics through `robo doctor`
- command wrapping through `robo run`, `robo sync`, and `robo develop`
- generated Nix plumbing

uv still owns Python:

- `.python-version`
- `.venv`
- `pyproject.toml`
- `uv.lock`
- Python package resolution

## Files You Usually Edit

Most users edit:

```text
pyproject.toml
uv.lock
.python-version
robo.nix
src/
scripts/
```

Most users should not hand-edit:

```text
flake.nix
flake.lock
```

Those files are visible and inspectable, but they are generated plumbing.

## Initialize A Project

For an existing repo:

```bash
cd /path/to/project
robo init .
```

In the alpha Nix-app form:

```bash
nix run github:ausbxuse/robo-nix#robo -- init .
```

`robo init`:

- keeps existing `pyproject.toml` and `uv.lock`
- writes `.python-version`
- writes `robo.nix`
- writes generated flake plumbing
- scans common Python dependencies and workspace paths for native runtime needs

## Debug Runtime Problems

Start cheap:

```bash
robo doctor
```

Ask why a component is present:

```bash
robo doctor --why
```

Use deeper runtime probes when Python packages are synced and native imports fail:

```bash
robo doctor --deep
```

Machine-readable audit output:

```bash
robo doctor --why --json
robo contract --json
```

## Vendor Sources

Some robotics projects need local vendor SDKs or patched upstream source trees.

Use:

```bash
robo vendor
```

`robo vendor` detects known local vendor checkouts, clones only modules with an explicit public `sourceUrl`, and runs configured bootstrap scripts. For proprietary or project-owned sources, it tells you where to place the checkout instead of fetching anything.

Focused vendor commands:

```bash
robo vendor list
robo vendor add third_party/some-sdk
robo vendor doctor
robo vendor bootstrap
```

## Common Fixes

If `robo doctor` says `.python-version` does not match `pyproject.toml`, make both match the project’s intended Python version.

If it says `flake.lock` may point at an old local checkout, run:

```bash
nix flake lock --update-input robo-nix
```

If GUI plotting fails after `robo sync`, run:

```bash
robo doctor --deep
```

For Matplotlib windows, use an explicit Qt backend:

```bash
MPLBACKEND=QtAgg robo run python graph.py
```
