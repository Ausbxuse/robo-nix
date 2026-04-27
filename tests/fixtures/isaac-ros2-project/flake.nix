{
  description = "Isaac ROS 2 downstream project fixture for robo-nix";

  nixConfig = {
    extra-substituters = ["https://ros.cachix.org"];
    extra-trusted-public-keys = ["ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo="];
  };

  inputs = {
    robo-nix.url = "github:ausbxuse/robo-nix";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "fixture";
      description = "Isaac ROS 2 fixture exercising mkProjectFlake";
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
      supportedSystems = ["x86_64-linux"];
      workspaceRoot = ".";
    };
}
