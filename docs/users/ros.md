# ROS

Use this page when a project uses the current ROS 2 Jazzy runtime components in `robo-nix`.

Current support is intentionally narrow:

- `ros2-jazzy` provides a ROS 2 Jazzy underlay, colcon tooling, `vcs`, CycloneDDS defaults, and the ROS setup environment.
- `ros-workspace` expects a workspace at `ros_ws/src` and exposes `ROBO_NIX_ROS_WS`.
- The `ros2-workspace` profile combines the base runtime, uv-managed Python, native build tools, `ros2-jazzy`, and `ros-workspace`.
- Supported systems for ROS 2 Jazzy are Linux only: `x86_64-linux` and `aarch64-linux`.

This is runtime infrastructure. It is not a ROS launcher, rosdep replacement, package registry, networking policy layer, or robot-specific bringup system.

## Basic Shape

The default workspace convention is:

```text
ros_ws/src
```

In a ROS project, `robo-nix` can provide:

- ROS 2 Jazzy environment setup
- colcon build tooling
- native build tools used by ROS packages
- CycloneDDS defaults
- predictable shell variables for the workspace path

The downstream project still owns:

- ROS packages and source repositories
- `rosdep` policy and package installation choices
- launch files
- robot-specific scripts
- ROS_DOMAIN_ID or ROS_LOCALHOST_ONLY choices when the defaults are wrong
- simulator, hardware, and networking orchestration

## Current Limits

Current ROS support has not been validated as a full robot bringup workflow.

Known limits:

- Only ROS 2 Jazzy is modeled as a first-class component.
- ROS networking behavior is host- and project-specific.
- `rosdep` flows are not managed by `robo`.
- Non-`ros_ws/src` layouts need explicit project handling.
- Cross-platform ROS support is not claimed; the ROS 2 Jazzy component is Linux-only.

TODO:

<div class="todo-list" role="list">
  <label class="todo-item" role="listitem">
    <input type="checkbox" disabled>
    <span>Add focused validation for a real ROS 2 workspace that builds with colcon and runs a simple node inside <code>robo shell</code>.</span>
  </label>
  <label class="todo-item" role="listitem">
    <input type="checkbox" disabled>
    <span>Document the recommended path for projects that need <code>rosdep</code>, custom DDS configuration, or non-default workspace layouts after those workflows are tested.</span>
  </label>
</div>
