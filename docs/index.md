---
layout: home

hero:
  text: Native runtime setup for uv-powered robot-learning projects.
  tagline: Use uv for Python packages.<br>Use robo for the native runtime.
  actions:
    - theme: brand
      text: Start Here
      link: /users/
    - theme: alt
      text: Why robo-nix
      link: /blog
features:
  - title: Enter the runtime
    details: Run robo up --shell, then use the project's normal uv sync and Python commands.
  - title: Keep Python in uv
    details: uv owns Python packages, virtual environments, dependency groups, indexes, editable sources, and uv.lock.
  - title: Put native pieces in Nix
    details: Nix supplies CUDA, graphics libraries, ROS tooling, simulators, compilers, FFmpeg, and shared libraries.
  - title: Debug the boundary
    details: robo check separates Python, runtime, host driver, graphics, CUDA, and native build failures.
---

## Quick Start

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh

robo up --shell
uv sync
robo run python -m pytest
```

`robo-nix` is for robot-learning projects that want reproducible native runtime setup without turning every contributor into a Nix user. The goal is easier downstream usage: less setup drift, clearer failures, and fewer environment details for each user to rediscover.

::: warning Early beta
`robo-nix` is still being validated against real robot-learning projects. CLI wording, generated files, diagnostics, runtime coverage, and installer behavior may change. Review generated `robo.nix` and `flake.nix` before committing them, and pin versions for shared team workflows.
:::

It keeps the contract simple:

- uv owns Python packages and virtual environments.
- Nix owns native libraries and runtime tooling.
- robo owns the commands, generated runtime files, and diagnostics.

## Where to Go

- New to the project: start with the [User Guide](./users/).
- Debugging setup: read [Troubleshooting](./users/troubleshooting.md).
- Runtime details: read [Runtime Support](./users/runtime.md).
- Maintaining robo-nix: read the [Developer Guide](./developers/).
- Contributing: read [Contributing](https://github.com/ausbxuse/robo-nix/blob/develop/CONTRIBUTING.md).
