{
  description = "Canonical robot-learning development environments with composable Nix components";

  nixConfig = {
    extra-substituters = ["https://ros.cachix.org"];
    extra-trusted-public-keys = ["ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo="];
  };

  inputs = {
    nix-ros-overlay.url = "github:lopsided98/nix-ros-overlay/master";
    nixpkgs.follows = "nix-ros-overlay/nixpkgs";
  };

  outputs = {
    nix-ros-overlay,
    nixpkgs,
    ...
  }: let
    inherit (nixpkgs) lib;
    componentCatalog = import ./components {inherit lib;};
    componentMetadata = import ./lib/component-metadata.nix;
    profileMetadata = import ./lib/profile-metadata.nix;
    runtimeInference = import ./lib/runtime-inference.nix;
    vendorMetadata = import ./lib/vendor-metadata.nix;
    presetEnvCatalog = import ./envs;
    allSystems = lib.unique (
      lib.concatMap
      (envSpec: envSpec.supportedSystems)
      (builtins.attrValues presetEnvCatalog)
    );
    roboLib = import ./lib {
      inherit componentCatalog componentMetadata lib nix-ros-overlay nixpkgs profileMetadata runtimeInference vendorMetadata;
    };
    generatedPresets = roboLib.mkFlakeFromEnvCatalog {
      defaultEnvName = "robot-learning";
      envs = presetEnvCatalog;
    };
    repoSupport = import ./lib/repo-support.nix {
      inherit allSystems componentMetadata lib nixpkgs profileMetadata runtimeInference vendorMetadata;
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
          cuda-doctor = {
            type = "app";
            program = "${repoPackages.${system}.cuda-doctor}/bin/cuda-doctor";
            meta.description = "Validate host CUDA and NVIDIA prerequisites";
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
          inherit vendorMetadata;
          envs = presetEnvCatalog;
        };
      packages = lib.mapAttrs (system: packages:
        packages // repoPackages.${system})
      generatedPresets.packages;
    };
}
