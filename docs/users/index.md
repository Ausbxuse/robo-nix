# User Guide

Use this section when you want to install `robo`, enter a project runtime, run commands, or debug environment failures.

## Start Here

- [Getting Started](./getting-started.md): install `robo`, prepare a project, and run the first Python command.
- [Workflow](./workflow.md): command reference for `robo up`, `robo shell`, `robo run`, `robo check`, `robo diagnose`, and `robo status`.
- [Python Boundary](./python.md): what uv owns, what Nix owns, and how to avoid Python/native ABI mixing.

## Troubleshooting

- [Diagnostics](./diagnostics.md): choose the right diagnostic command.
- [Runtime Failure Guide](./failure-guide.md): look up distinctive errors such as `GLIBC_2.38 not found`, `Qt6Config.cmake`, or `EGL: Failed to get EGL display`.

## Runtime Capabilities

These pages explain what `robo-nix` can provide, what the host still owns, and which parts are not solved yet.

- [CUDA](./cuda.md): Python CUDA wheels, native CUDA builds, and host driver visibility.
- [Graphics](./graphics.md): OpenGL, EGL, Qt, display variables, and current host-driver limits.
- [ROS](./ros.md): current ROS 2 Jazzy workspace support and unvalidated ROS gaps.
