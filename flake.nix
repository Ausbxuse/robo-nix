{
  description = "Canonical robot-learning development environments with composable Nix components";

  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = ["https://ros.cachix.org"];
    extra-trusted-public-keys = ["ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo="];
  };

  inputs = {
    nix-ros-overlay.url = "github:lopsided98/nix-ros-overlay/master";
    nixpkgs-python = {
      url = "github:cachix/nixpkgs-python";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.follows = "nix-ros-overlay/nixpkgs";
  };

  outputs = {
    nix-ros-overlay,
    nixpkgs-python,
    nixpkgs,
    ...
  }: let
    inherit (nixpkgs) lib;
    componentCatalog = import ./nix/modules {inherit lib;};
    componentMetadata = import ./nix/metadata/components.nix;
    profileMetadata = import ./nix/metadata/profiles.nix;
    runtimeInference = import ./nix/metadata/runtime-inference.nix;
    presetEnvCatalog = import ./nix/presets.nix;
    allSystems = lib.unique (
      lib.concatMap
      (envSpec: envSpec.supportedSystems)
      (builtins.attrValues presetEnvCatalog)
    );
    roboLib = import ./nix {
      inherit componentCatalog componentMetadata lib nix-ros-overlay nixpkgs nixpkgs-python profileMetadata runtimeInference;
    };
    generatedPresets = roboLib.mkFlakeFromEnvCatalog {
      defaultEnvName = "robot-learning";
      envs = presetEnvCatalog;
    };
    repoSupport = import ./nix/repo-support.nix {
      inherit allSystems componentMetadata lib nixpkgs profileMetadata runtimeInference;
      repoRoot = ./.;
    };
    inherit (repoSupport) repoChecks repoPackages;
  in
    generatedPresets
    // {
      apps = lib.mapAttrs (system: apps:
        apps
        // {
          repo-fmt = {
            type = "app";
            program = "${repoPackages.${system}.repo-fmt}/bin/repo-fmt";
            meta.description = "Format robo-nix sources";
          };
          repo-lint = {
            type = "app";
            program = "${repoPackages.${system}.repo-lint}/bin/repo-lint";
            meta.description = "Lint robo-nix sources";
          };
          repo-profile = {
            type = "app";
            program = "${repoPackages.${system}.repo-profile}/bin/repo-profile";
            meta.description = "Profile robo-nix evaluation paths";
          };
          cuda-check = {
            type = "app";
            program = "${repoPackages.${system}.cuda-check}/bin/cuda-check";
            meta.description = "Validate host CUDA and NVIDIA prerequisites";
          };
          docs-serve = {
            type = "app";
            program = "${repoPackages.${system}.docs-serve}/bin/docs-serve";
            meta.description = "Preview the robo-nix documentation site";
          };
          robo = {
            type = "app";
            program = "${repoPackages.${system}.robo}/bin/robo";
            meta.description = "Initialize and manage downstream robo-nix runtime files";
          };
        })
      generatedPresets.apps;
      checks =
        lib.mapAttrs
        (system: checks:
          checks // repoChecks.${system})
        generatedPresets.checks;
      formatter = lib.mapAttrs (_: packages: packages.repo-fmt) repoPackages;
      lib =
        roboLib
        // {
          components = componentCatalog;
          inherit componentMetadata;
          inherit profileMetadata;
          inherit runtimeInference;
          envs = presetEnvCatalog;
        };
      packages = lib.mapAttrs (system: packages:
        packages // repoPackages.${system})
      generatedPresets.packages;
    };
}
