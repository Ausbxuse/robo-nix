---
layout: home

hero:
  name: robo-nix
  text: Reproducible robotics environments without the setup ritual.
  tagline: Keep Python normal with uv. Let Nix handle native runtime libraries. Use robo for the workflow.
  actions:
    - theme: brand
      text: Get Started
      link: /users/getting-started
    - theme: alt
      text: Why robo-nix
      link: /blog

features:
  - title: One workflow for setup
    details: Prepare a project, enter the runtime, run commands, and inspect problems through the robo CLI.
  - title: Normal Python, stronger runtime
    details: uv keeps owning .python-version, pyproject.toml, uv.lock, and .venv while Nix supplies native libraries.
  - title: Built for robotics friction
    details: CUDA, graphics, ROS, simulators, compilers, media libraries, and host-driver diagnostics belong in the environment story.
  - title: Honest diagnostics
    details: robo separates Python, Nix runtime, host driver, and project bootstrap failures instead of hiding them behind fragile shell tricks.
---

## What It Feels Like

```bash
curl -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/develop/scripts/install.sh | sh

robo up --shell
uv sync
robo run python -m pytest
```

`robo-nix` is for robot-learning projects that want reproducible native runtime setup without turning every contributor into a Nix user.

::: warning Early beta
`robo-nix` is still being validated against real robotics projects. CLI wording, generated files, diagnostics, runtime coverage, and installer behavior may change. Review generated `robo.nix` and `flake.nix` before committing them, and pin versions for shared team workflows.
:::

It keeps the contract simple:

- uv owns Python packages and virtual environments.
- Nix owns native libraries and runtime tooling.
- robo owns the commands, generated runtime files, and diagnostics.

## Where to Go

- New to the project: start with the [User Guide](./users/).
- Debugging setup: read [Diagnostics](./users/diagnostics.md) or the [Runtime Failure Guide](./users/failure-guide.md).
- Maintaining robo-nix: read the [Developer Guide](./developers/).
