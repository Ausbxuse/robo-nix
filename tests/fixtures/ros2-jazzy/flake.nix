{
  description = "ROS 2 Jazzy downstream project fixture for robo-nix";

  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = [
      "https://nixpkgs-python.cachix.org"
      "https://ros.cachix.org"
    ];
    extra-trusted-public-keys = [
      "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
      "ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo="
    ];
  };

  inputs = {
    robo-nix.url = "github:ausbxuse/robo-nix";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "fixture";
      description = "ROS 2 Jazzy fixture exercising mkProjectFlake";
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
}
