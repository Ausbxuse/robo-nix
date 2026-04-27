{
  componentCatalog,
  componentMetadata,
  lib,
  nix-ros-overlay,
  nixpkgs,
  profileMetadata,
  runtimeInference,
  vendorMetadata,
}: let
  mkEngine = import ./mk-flake-from-envs.nix {
    inherit componentCatalog lib nix-ros-overlay nixpkgs;
  };
  loadProjectManifest = import;
  mkProjectFlakeFromManifest = manifestPath:
    mkEngine.mkProjectFlake (loadProjectManifest manifestPath);
in {
  inherit (mkEngine) mkFlakeFromEnvCatalog mkProjectFlake normalizeEnvSpec;
  inherit loadProjectManifest mkProjectFlakeFromManifest;
  inherit componentMetadata;
  inherit profileMetadata;
  inherit runtimeInference;
  inherit vendorMetadata;
}
