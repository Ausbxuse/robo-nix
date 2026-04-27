{common}: {
  ros2-jazzy = {
    pkgs,
    pkgsRos,
    ...
  }: let
    rosDistro = "jazzy";
    rosPkgs = pkgsRos.rosPackages.${rosDistro};
    rosEnv = rosPkgs.ros-environment;
  in {
    packages = [
      pkgsRos.python3Packages.colcon-core
      pkgsRos.python3Packages.colcon-cmake
      pkgsRos.python3Packages.colcon-ros
      pkgs.vcs
      pkgs.cyclonedds
      rosEnv
    ];
    shellInit =
      common.exportVars {
        ROBO_NIX_ROS_DISTRO = rosDistro;
        ROBO_NIX_ROS_UNDERLAY = rosEnv;
      }
      + "\n"
      + common.exportDefaults {
        ROS_DOMAIN_ID = "0";
        ROS_LOCALHOST_ONLY = "0";
        RMW_IMPLEMENTATION = "rmw_cyclonedds_cpp";
      }
      + "\n"
      + common.sourceIfPresent "$ROBO_NIX_ROS_UNDERLAY/setup.bash";
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "ros2-jazzy" [];
  };

  ros-workspace = _: {
    shellInit = common.exportVars {
      ROBO_NIX_ROS_WS = "$WORKSPACE_ROOT/ros_ws";
      ROBO_NIX_ROS_WS_SETUP = "$ROBO_NIX_ROS_WS/install/setup.bash";
    };
    requiredDirectories = ["ros_ws/src"];
    check = common.mkComponentCheck "ros-workspace" ["required_dir=ros_ws/src"];
  };
}
