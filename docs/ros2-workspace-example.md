# ROS 2 Workspace Example

This example is for a project that needs a standard ROS 2 workspace with `ros_ws/src`, `colcon`, and a reproducible Python and native-tooling base.

It maps to the maintained `ros2-workspace` profile.

## When To Use This

Use this path when:

- you are starting a new ROS 2 robotics project
- you want `colcon` and the usual workspace layout
- you do not need Isaac Sim yet
- you want the easiest ROS-oriented `robo-nix` entrypoint

## Fastest Path

Create a new directory and generate the local project files:

```bash
mkdir ros2-project
cd ros2-project
nix run github:ausbxuse/robo-nix#robo -- init . \
  --name ros2-project \
  --profile ros2-workspace
```

That writes:

- `flake.nix`
- `robo.nix`
- `ros_ws/src`

## Validate The Workspace

Run the generated checks before adding packages:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
```

Then enter the environment:

```bash
nix develop
```

## Add Your ROS 2 Packages

Put your ROS packages under:

```text
ros_ws/src/
```

Then use the shell as your normal ROS 2 workspace:

```bash
cd ros_ws
colcon build
```

If your project also uses Python dependencies, manage those with `uv` inside the same shell.

## Interactive Version

If you want prompts instead of flags:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --interactive
```

Then answer:

- advanced/manual setup: `no`
- ROS 2 workspace: `yes`
- project setup: `None`

That lands on the same maintained `ros2-workspace` profile.

## Generated Manifest Shape

The generated `robo.nix` will look roughly like:

```nix
{
  envName = "ros2-project";
  description = "ROS 2 Jazzy workspace with colcon and ros_ws/src layout.";
  components = [
    "base"
    "python-uv"
    "native-build"
    "ros2-jazzy"
    "ros-workspace"
  ];
  pythonVersion = "3.11";
  supportedSystems = [
    "x86_64-linux"
    "aarch64-linux"
  ];
  workspaceRoot = ".";
}
```

## What To Do Next

If this project later needs simulation or GPU-heavy tooling:

- move to the `isaac-ros2` profile for Isaac Sim
- or extend `robo.nix` with additional reusable components
