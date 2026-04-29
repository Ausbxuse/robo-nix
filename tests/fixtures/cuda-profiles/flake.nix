{
  description = "CUDA profile downstream project fixture for robo-nix";

  inputs = {
    robo-nix.url = "github:ausbxuse/robo-nix";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "fixture";
      description = "CUDA fixture exercising mkProjectFlake";
      components = [
        "base"
        "python-uv"
        "native-build"
        "cuda-toolkit"
      ];
      pythonVersion = "3.11";
      supportedSystems = ["x86_64-linux"];
      workspaceRoot = ".";
    };
}
