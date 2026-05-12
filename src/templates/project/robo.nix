{
  components = [
{{components}}
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];

  # Host graphics wrapper policy.
  # Options:
  #   "auto"   - use /run/opengl-driver on NixOS hosts and robo-provided nixGL
  #              wrappers on other Linux hosts.
  #   null     - do not import a host graphics wrapper; use component defaults.
  #   "nixgl" - import graphics variables from a nixGL wrapper;
  #              set ROBO_NIX_NIXGL for uncommon wrapper names or locations.
  #   "nixgl-nvidia" - import graphics variables from nixGLNvidia only.
  #   "nvidia" - compatibility alias for "nixgl-nvidia".
  hostGraphics = "auto";
}
