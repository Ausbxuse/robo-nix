# ROS

ROS support currently centers on the `ros2-jazzy` and `ros-workspace` components in `nix/modules/ros.nix`.

The default ROS workspace convention is:

```text
ros_ws/src
```

Additional ROS distributions should be added only when their component contract is reusable across real downstream projects.
