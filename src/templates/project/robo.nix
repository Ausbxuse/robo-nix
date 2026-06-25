{
  defaultProfile = "default";

  profiles = {
    default = {
      components = [
{{components}}
      ];

      pythonExtras = [
      ];

      pythonGroups = [
      ];

      extraPackages = pkgs: [
      ];

      extraRuntimeLibraries = pkgs: [
      ];

      # CUDA extension build architecture hints.
      # Use null to leave build systems alone, "auto" to best-effort detect
      # local NVIDIA GPUs, or an explicit list such as [ "8.6" "8.9" ].
      cudaArchitectures = null;

      # Host graphics wrapper policy.
      # Options:
      #   "auto"   - use /run/opengl-driver on NixOS hosts and robo-provided nixGL
      #              wrappers on other Linux hosts.
      #   null     - do not import a host graphics wrapper; use component defaults.
      #   "nixgl" - import graphics variables from a nixGL wrapper;
      #              set ROBO_NIX_NIXGL for uncommon wrapper names or locations.
      #   "nixgl-nvidia" - import graphics variables from nixGLNvidia only.
      hostGraphics = "auto";
    };
  };
}
