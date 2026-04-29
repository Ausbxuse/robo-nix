# User-facing component catalog metadata. Add entries here when a runtime
# component exists in nix/modules and should be visible to `robo init --list-components`.
{
  base = {
    category = "core";
    description = "Common CLI tooling and environment variables used by most projects.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };

  python-uv = {
    category = "core";
    description = "uv-based Python version, virtualenv, and package workflow support.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };

  native-build = {
    category = "core";
    description = "C/C++ build toolchain for native extensions and robotics libraries.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };

  media = {
    category = "runtime";
    description = "FFmpeg runtime for video, PyAV, dataset IO, and teleoperation media paths.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };

  linux-headers = {
    category = "native";
    description = "Linux kernel headers for native extensions that include kernel or network headers.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  x11-gl = {
    category = "ui";
    description = "Linux desktop display, OpenGL, font, DBus, and XCB runtime libraries for GUI and simulator workloads.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  qt6 = {
    category = "ui";
    description = "Qt 6 runtime support for robotics desktop tools and services.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  ros2-jazzy = {
    category = "ros";
    description = "ROS 2 Jazzy underlay with colcon and CycloneDDS defaults.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  ros-workspace = {
    category = "ros";
    description = "ROS workspace layout expectations for projects using ros_ws/src.";
    scaffoldDirectories = [
      "ros_ws/src"
    ];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };

  mujoco = {
    category = "simulation";
    description = "MuJoCo simulator package and runtime environment variables.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  cuda-toolkit = {
    category = "gpu";
    description = "CUDA compiler, headers, and native extension build support for GPU workloads.";
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
    ];
  };

  isaac-sim = {
    category = "simulation";
    description = "Isaac Sim workspace integration with third_party/isaac-sim layout.";
    scaffoldDirectories = [
      "third_party/isaac-sim"
    ];
    supportedSystems = [
      "x86_64-linux"
    ];
  };
}
