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
  #   "nvidia" - use the host NVIDIA graphics provider. robo-nix selects known
  #              NixOS and Ubuntu-style manifest paths; set ROBO_NIX_NVIDIA_VK_ICD
  #              or ROBO_NIX_NVIDIA_EGL_VENDOR for uncommon host layouts.
  hostGraphics = null;
}
