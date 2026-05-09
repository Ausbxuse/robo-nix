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
  #   null      - leave Vulkan/EGL/GLX provider selection to the host session.
  #   "nvidia" - use the host NVIDIA graphics provider, useful for Isaac Sim
  #              or RTX rendering on NixOS and hybrid-GPU machines.
  hostGraphics = null;
}
