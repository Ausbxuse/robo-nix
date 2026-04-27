let
  profiles = import ../lib/profile-metadata.nix;

  # NOTE: repo presets are convenience outputs for demos and checks. The
  # scalable downstream API is still mkProjectFlake/mkProjectFlakeFromManifest.
  fromProfile = profile: overrides: profile // overrides;
in {
  robot-learning = fromProfile profiles.minimal {
    description = "Minimal reusable robot-learning shell";
  };

  ros2-learning = fromProfile profiles.ros2-workspace {
    description = "ROS 2 Jazzy workspace shell for robot-learning projects";
  };

  mujoco-learning = fromProfile profiles.mujoco-sim {
    description = "MuJoCo-based robot-learning shell";
  };

  isaac-ros2-learning = fromProfile profiles.isaac-ros2 {
    description = "Isaac Sim plus ROS 2 Jazzy shell for robot-learning projects";
  };

  gpu-learning = fromProfile profiles.minimal {
    description = "CUDA-enabled robot-learning shell";
    components = profiles.minimal.components ++ ["cuda-toolkit"];
    supportedSystems = ["x86_64-linux"];
  };

  # TODO(robo): decide whether this remains a public preset after real
  # downstream Isaac usage proves whether non-ROS Isaac is common enough.
  isaac-learning = fromProfile profiles.mujoco-sim {
    description = "Isaac Sim oriented shell with local host install integration";
    components = [
      "base"
      "python-uv"
      "native-build"
      "x11-gl"
      "cuda-toolkit"
      "isaac-sim"
    ];
    supportedSystems = ["x86_64-linux"];
  };
}
