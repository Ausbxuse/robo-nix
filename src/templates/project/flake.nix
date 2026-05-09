{
  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = [
      "https://nixpkgs-python.cachix.org"
    ];
    extra-trusted-public-keys = [
      "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
    ];
  };

  inputs.robo-nix.url = "{{robo_nix_url}}";

  # NOTE: generated plumbing. Most users should edit robo.nix,
  # pyproject.toml, and .python-version instead of this file.
  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}
