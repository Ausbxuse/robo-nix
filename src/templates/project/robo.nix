{
  components = [
{{components}}
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];

  # Host graphics provider policy.
  # Options:
  #   null      - do not select a host graphics provider; use component defaults.
  #   "nvidia" - use the host NVIDIA graphics provider. robo-nix selects known
  #              NixOS and Ubuntu-style manifest paths and prepares the matching
  #              NVIDIA GLVND vendor library, EGL platform, and GBM backend view; set
  #              ROBO_NIX_NVIDIA_VK_ICD, ROBO_NIX_NVIDIA_EGL_VENDOR, or
  #              ROBO_NIX_NVIDIA_DRIVER_LIB_DIR for uncommon host layouts.
  #   "nixgl"  - import graphics variables from a nixGL wrapper found on PATH;
  #              set ROBO_NIX_NIXGL for uncommon wrapper names or locations.
  hostGraphics = null;
}
