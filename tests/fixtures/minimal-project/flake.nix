{
  description = "Minimal downstream project fixture for robo-nix";

  inputs = {
    robo-nix.url = "github:ausbxuse/robo-nix";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "fixture";
      description = "Minimal fixture exercising mkProjectFlake";
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
}
