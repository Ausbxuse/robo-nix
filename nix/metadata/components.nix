# User-facing component catalog metadata. Add entries here when a runtime
# component exists in nix/modules and should be visible to `robo init --list-components`.
{
  base = {
    category = "core";
    description = "Common CLI tooling and environment variables used by most projects.";
    provides = ["runtime.shell.base"];
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
    description = "Nix-managed CPython interpreter plus uv version, virtualenv, and package workflow support.";
    provides = [
      "runtime.python.uv"
      "runtime.python.interpreter"
    ];
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
    description = "C/C++ build toolchain for native extensions and robot-learning libraries.";
    provides = [
      "runtime.native.compiler"
      "runtime.native.cmake"
      "runtime.native.pkg-config"
      "runtime.native.libstdcxx"
    ];
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
    provides = [
      "runtime.media.ffmpeg"
      "runtime.media.video-io"
    ];
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
    provides = ["runtime.native.linux-headers"];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  x11-gl = {
    category = "ui";
    description = "Linux X11/XWayland desktop OpenGL, font, DBus, and XCB runtime libraries for GUI and simulator workloads.";
    provides = [
      "runtime.graphics.egl"
      "runtime.graphics.opengl"
      "runtime.graphics.vulkan-loader"
      "runtime.graphics.x11"
    ];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  wayland-gl = {
    category = "ui";
    description = "Linux Wayland desktop OpenGL, font, DBus, and Wayland runtime libraries for GUI and simulator workloads.";
    provides = [
      "runtime.graphics.egl"
      "runtime.graphics.opengl"
      "runtime.graphics.vulkan-loader"
      "runtime.graphics.wayland"
    ];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  qt6 = {
    category = "ui";
    description = "Qt 6 runtime support for robot-learning desktop tools and services.";
    provides = ["runtime.ui.qt6"];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  matplotlib-qt = {
    category = "ui";
    description = "Matplotlib QtAgg backend selection for projects that use Qt Python bindings.";
    provides = ["runtime.ui.matplotlib-qt"];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  ros2-jazzy = {
    category = "ros";
    description = "ROS 2 Jazzy underlay with colcon and CycloneDDS defaults.";
    provides = [
      "runtime.ros.colcon"
      "runtime.ros.ros2-jazzy"
    ];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  ros-workspace = {
    category = "ros";
    description = "ROS workspace layout expectations for projects using ros_ws/src.";
    provides = ["runtime.ros.workspace"];
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
    provides = ["runtime.sim.mujoco"];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };

  cuda-toolkit = {
    category = "gpu";
    description = "CUDA compiler, headers, and native extension build support for GPU workloads.";
    provides = [
      "runtime.cuda.headers"
      "runtime.cuda.link.cudart"
      "runtime.cuda.nvcc"
      "runtime.cuda.toolkit"
    ];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
    ];
  };

  isaac-sim = {
    category = "simulation";
    description = "Isaac Sim runtime environment hooks.";
    provides = ["runtime.sim.isaac"];
    scaffoldDirectories = [];
    supportedSystems = [
      "x86_64-linux"
    ];
  };
}
