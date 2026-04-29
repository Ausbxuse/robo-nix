{
  minimal = {
    description = "Minimal Python robotics environment with common CLI tooling and native build support.";
    components = [
      "base"
      "python-uv"
      "native-build"
    ];
    pythonVersion = "3.11";
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    workspaceRoot = ".";
  };

  ros2-workspace = {
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
  };

  mujoco-sim = {
    description = "MuJoCo-focused simulation environment for Linux research workstations.";
    components = [
      "base"
      "python-uv"
      "native-build"
      "x11-gl"
      "mujoco"
    ];
    pythonVersion = "3.11";
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    workspaceRoot = ".";
  };

  isaac-ros2 = {
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
  };
}
