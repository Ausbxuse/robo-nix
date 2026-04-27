# Isaac Sim + ROS 2 Example

This example is for a Linux project that needs Isaac Sim, ROS 2 Jazzy, and host CUDA integration.

It maps to the maintained `isaac-ros2` profile.

## When To Use This

Use this path when:

- you are building a simulator-heavy robotics project
- you need both Isaac Sim and ROS 2
- you are on `x86_64-linux`
- you want a guided starting point instead of hand-assembling the environment

## What This Example Sets Up

- a generated `flake.nix`
- a generated `robo.nix`
- `ros_ws/src/`
- `third_party/isaac-sim/`
- Linux-only platform gating
- uv-managed Python `3.11`
- host CUDA validation through the existing doctor path

## Fastest Path

Create the project directory and generate the local adapter files:

```bash
mkdir isaac-ros2-project
cd isaac-ros2-project
nix run github:ausbxuse/robo-nix#robo -- init . \
  --name isaac-ros2-project \
  --profile isaac-ros2
```

That writes:

- `flake.nix`
- `robo.nix`
- `ros_ws/src`
- `third_party/isaac-sim`

## Validate Before Bootstrapping

Run the generated checks:

```bash
nix run .#default -- --doctor
nix run .#default -- --dry-run
```

Then enter the environment:

```bash
nix develop
```

`--doctor` is especially important here because this profile depends on Linux host behavior and CUDA-related assumptions.

## Fill In The Workspace

Put your ROS packages under:

```text
ros_ws/src/
```

Put your local Isaac Sim checkout or host-managed installation bridge under:

```text
third_party/isaac-sim/
```

The generated app validates that those paths exist before the bootstrap flow continues.

## Interactive Version

If you prefer the wizard:

```bash
nix run github:ausbxuse/robo-nix#robo -- init --interactive
```

Then answer:

- advanced/manual setup: `no`
- ROS 2 workspace: `yes`
- project setup: `Isaac Sim`

That maps to the same maintained `isaac-ros2` profile.

## Generated Manifest Shape

The generated `robo.nix` will look roughly like:

```nix
{
  envName = "isaac-ros2-project";
  description = "Isaac Sim plus ROS 2 Jazzy with CUDA host integration.";
  components = [
    "base"
    "python-uv"
    "native-build"
    "x11-gl"
    "cuda-toolkit"
    "isaac-sim"
    "ros2-jazzy"
    "ros-workspace"
  ];
  pythonVersion = "3.11";
  supportedSystems = [
    "x86_64-linux"
  ];
  workspaceRoot = ".";
}
```

## Operational Notes

- this is intentionally Linux-first
- CUDA remains host-integrated rather than fully packaged here
- if you only need a ROS 2 workspace, start with the simpler ROS 2 example instead

That keeps the beginner path simpler and avoids unnecessary GPU or simulator overhead.
