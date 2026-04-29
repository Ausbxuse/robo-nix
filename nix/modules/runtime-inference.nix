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
      dependencies = ["torchvision"];
      components = ["media"];
      note = "TorchVision video and dataset IO commonly need media runtime libraries";
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
        "pytorch3d"
        "torch3d"
        "jax"
        "jaxlib"
        "triton"
      ];
      components = ["native-build"];
      note = "ML packages often build or load native extensions";
    }
    {
      dependencies = [
        "cuda-python"
        "cupy"
        "cupy-cuda11x"
        "cupy-cuda12x"
        "flash-attn"
        "pytorch3d"
        "torch3d"
      ];
      components = ["cuda-toolkit"];
      note = "CUDA Python packages and CUDA extension builds need host CUDA integration";
    }
    {
      dependencies = ["flash-attn"];
      components = ["native-build"];
      note = "FlashAttention builds CUDA native extensions";
    }
    {
      dependencies = ["evdev"];
      components = [
        "linux-headers"
        "native-build"
      ];
      note = "evdev native extensions include Linux input headers";
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
      note = "workspace contains Qt service paths";
    }
  ];

  scriptDiscovery = {
    roots = ["scripts"];
    names = [];
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
      note = "bootstrap script references Qt GUI runtime";
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
