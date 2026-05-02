# ROS

ROS support currently centers on reusable runtime components, especially `ros2-jazzy` and `ros-workspace`.

The default workspace convention is:

```text
ros_ws/src
```

`robo-nix` supports ROS as native runtime infrastructure. It is not a project-specific ROS launcher or package policy layer.

## What Nix Should Provide

For ROS projects, Nix can provide:

- ROS distribution packages
- ROS setup environment
- colcon and native build tools
- system libraries needed by ROS packages
- runtime hooks for a predictable shell

## What the Project Owns

The downstream project owns:

- workspace layout beyond the common convention
- package selection
- source repositories
- rosdep policy
- launch files
- robot-specific setup scripts
- simulator-specific orchestration

Additional ROS distributions should be added only when their component contract is reusable across real downstream projects.
