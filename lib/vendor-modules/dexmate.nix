{
  dexmate-gmr = {
    description = "Dexmate GMR retargeting checkout with local Vega-1 robot patch support.";
    installPath = "third_party/GMR";
    detectPaths = ["third_party/GMR"];
    sourceUrl = null;
    components = [
      "mujoco"
      "native-build"
    ];
    requiredPaths = [
      "setup.py"
    ];
    bootstrapScripts = [
      "scripts/apply_vendor_patches.sh"
      "scripts/bootstrap_gmr_env.sh"
    ];
    patches = [
      "third_party/vendor-patches/gmr-dexmate-vega1-addon.patch"
    ];
  };

  dexmate-xrobot-pc-service = {
    description = "XRobo Toolkit PC service source used for PICO/XRobo teleoperation.";
    installPath = "third_party/XRoboToolkit-PC-Service";
    detectPaths = ["third_party/XRoboToolkit-PC-Service"];
    sourceUrl = null;
    components = [
      "native-build"
      "qt6"
      "linux-headers"
    ];
    requiredPaths = [
      "RoboticsService/CMakeLists.txt"
      "RoboticsService/PXREARobotSDK/build.sh"
    ];
    bootstrapScripts = [
      "scripts/apply_vendor_patches.sh"
      "scripts/bootstrap_xrobot_pc_service.sh"
    ];
    patches = [
      "third_party/vendor-patches/xrobottoolkit-pc-service-standalone-nix.patch"
    ];
  };

  dexmate-xrobot-pybind = {
    description = "Python bindings for the XRobo Toolkit SDK used by Dexmate GMR mode.";
    installPath = "third_party/XRoboToolkit-PC-Service-Pybind";
    detectPaths = ["third_party/XRoboToolkit-PC-Service-Pybind"];
    sourceUrl = null;
    components = [
      "native-build"
      "linux-headers"
    ];
    requiredPaths = [
      "setup.py"
      "bindings/py_bindings.cpp"
      "CMakeLists.txt"
    ];
    bootstrapScripts = [
      "scripts/bootstrap_xrobot_sdk.sh"
    ];
    patches = [];
  };

  dexmate-mobile-assets = {
    description = "Dexmate Vega GMR addon assets and configs consumed by the GMR checkout.";
    installPath = "third_party/mobile_dex_teleop";
    detectPaths = ["third_party/mobile_dex_teleop"];
    sourceUrl = null;
    components = [];
    requiredPaths = [];
    bootstrapScripts = [];
    patches = [];
  };

  dexmate-vega-navigation-stack = {
    description = "Vega navigation stack checkout used by the robot-side SLAM/navigation workflow.";
    installPath = "third_party/vega-navigation-stack";
    detectPaths = ["third_party/vega-navigation-stack"];
    sourceUrl = null;
    components = [
      "ros2-jazzy"
    ];
    requiredPaths = [
      "README.md"
    ];
    bootstrapScripts = [];
    patches = [];
  };

  dexmate-cartographer-ros-vega = {
    description = "Dexmate fork of Cartographer ROS for Vega SLAM/container workflows.";
    installPath = "third_party/cartographer_ros_vega";
    detectPaths = ["third_party/cartographer_ros_vega"];
    sourceUrl = null;
    components = [
      "ros2-jazzy"
    ];
    requiredPaths = [
      "cartographer_ros.rosinstall"
    ];
    bootstrapScripts = [];
    patches = [];
  };
}
