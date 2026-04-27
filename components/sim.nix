{common}: {
  mujoco = {pkgs, ...}: {
    packages = [
      pkgs.mujoco
    ];
    shellInit =
      common.exportVars {
        MUJOCO_PATH = pkgs.mujoco;
      }
      + "\n"
      + common.exportDefaults {
        MUJOCO_GL = "egl";
      };
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "mujoco" [];
  };

  cuda-toolkit = _: {
    shellInit =
      common.exportDefaults {
        CUDA_PATH = "\${ROBO_NIX_CUDA_ROOT:-/usr/local/cuda}";
        NVIDIA_VISIBLE_DEVICES = "all";
      }
      + "\n"
      + common.exportVars {
        CUDA_HOME = "$CUDA_PATH";
      }
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "/run/opengl-driver/lib";
    supportedSystems = common.x86LinuxSystems;
    gpuRequired = true;
    check = common.mkComponentCheck "cuda-toolkit" [];
  };

  isaac-sim = _: {
    shellInit = common.exportVars {
      ISAAC_SIM_ROOT = "$WORKSPACE_ROOT/third_party/isaac-sim";
      OMNI_KIT_ROOT = "$ISAAC_SIM_ROOT";
    };
    requiredDirectories = ["third_party/isaac-sim"];
    supportedSystems = common.x86LinuxSystems;
    check = common.mkComponentCheck "isaac-sim" ["required_dir=third_party/isaac-sim"];
  };
}
