# User Guide

This section is for people using `robo` in a project.

You do not need to learn Nix first. The important boundary is:

- `uv` owns Python packages, virtual environments, dependency groups, and `uv.lock`.
- Nix owns native libraries, CUDA, graphics, ROS, compilers, and shell environment.
- `robo` owns setup commands, runtime shells, command wrapping, and diagnostics.

## Start Here

If you are setting up a project for the first time:

1. Read [Getting Started](./getting-started.md).
2. Use [Workflow](./workflow.md) for daily commands.
3. Read [Python Boundary](./python.md) before changing virtualenv or lockfile behavior.

## When Something Breaks

Use [Diagnostics](./diagnostics.md) to choose between:

- `robo check` for probing the current project/runtime/host.
- `robo diagnose` for classifying an existing error log.
- `robo check --deep` for slower runtime probes.

Use the [Runtime Failure Guide](./failure-guide.md) when you already have a distinctive error phrase such as `GLIBC_2.38 not found`, `Qt6Config.cmake`, or `EGL: Failed to get EGL display`.

## Runtime Topics

Use these pages when a project needs a specific runtime area:

- [CUDA](./cuda.md)
- [Graphics](./graphics.md)
- [ROS](./ros.md)
