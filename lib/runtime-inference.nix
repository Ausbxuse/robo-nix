{
  defaultProfile = "minimal";

  dependencyRules = [
    {
      dependencies = [
        "mujoco"
        "dm-control"
        "gymnasium-robotics"
      ];
      components = [
        "mujoco"
        "x11-gl"
      ];
      note = "pyproject.toml uses MuJoCo/simulation packages";
    }
    {
      dependencies = [
        "glfw"
        "pyglet"
      ];
      components = ["x11-gl"];
      note = "pyproject.toml uses OpenGL windowing packages";
    }
    {
      dependencies = [
        "opencv-python"
        "opencv-contrib-python"
      ];
      components = [
        "x11-gl"
        "media"
      ];
      note = "OpenCV wheels commonly need graphics and media runtime libraries";
    }
    {
      dependencies = [
        "av"
        "pyav"
        "imageio-ffmpeg"
        "ffmpeg-python"
        "decord"
      ];
      components = ["media"];
      note = "pyproject.toml uses FFmpeg/media packages";
    }
    {
      dependencies = ["lerobot"];
      components = [
        "media"
        "x11-gl"
      ];
      note = "LeRobot workflows commonly need media and graphics runtime libraries";
    }
    {
      dependencies = [
        "pyside6"
        "pyqt6"
        "pyqt5"
      ];
      components = [
        "qt6"
        "x11-gl"
      ];
      note = "pyproject.toml uses Qt bindings that commonly need desktop display and OpenGL runtime libraries";
    }
    {
      dependencies = [
        "torch"
        "torchvision"
        "jax"
        "jaxlib"
        "flash-attn"
        "triton"
      ];
      components = ["native-build"];
      note = "ML packages often build or load native extensions";
    }
  ];

  workspaceDirectoryRules = [
    {
      root = "third_party";
      nameContains = [
        "xrobot"
        "qt"
      ];
      components = [
        "qt6"
        "linux-headers"
      ];
      note = "workspace contains Qt/vendor service paths";
    }
  ];

  scriptDiscovery = {
    roots = ["scripts"];
    names = ["apply_vendor_patches.sh"];
    prefixes = ["bootstrap_"];
    daemonTextContains = [
      "systemctl "
      "ros2 launch"
      "sleep infinity"
    ];
    checkoutFunction = "source_checkout_ready";
    pathRoot = "third_party/";
  };

  scriptRules = [
    {
      textContains = [
        "qt"
        "xrobot"
      ];
      components = ["qt6"];
      note = "bootstrap script references Qt/vendor GUI runtime";
    }
    {
      textContains = [
        "linux/"
        "linuxheaders"
        "linux-headers"
      ];
      components = ["linux-headers"];
      note = "bootstrap script references Linux headers";
    }
  ];
}
