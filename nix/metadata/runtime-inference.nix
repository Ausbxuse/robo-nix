# Add rules here when a common Python package reliably
# implies reusable native runtime support. Keep rules generic: do not encode
# project-specific uv groups, extras, source pins, package indexes, or install
# modes here. `note` is shown to users by `robo init`, so write it as concise
# public text, not internal commentary.
#
# CUDA wheel ABI requirements are inferred from uv.lock by the CLI. Do not add
# cuda-toolkit just because a wheel is CUDA-enabled; reserve cuda-toolkit for
# native CUDA extension build/link support.
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
        "desktop-gl"
      ];
      requires = [
        "runtime.sim.mujoco"
        "runtime.graphics.egl"
        "runtime.graphics.opengl"
        "runtime.native.libstdcxx"
      ];
      note = "pyproject.toml uses MuJoCo/simulation packages";
    }
    {
      dependencies = [
        "glfw"
        "pyglet"
      ];
      components = ["desktop-gl"];
      requires = [
        "runtime.graphics.egl"
        "runtime.graphics.opengl"
      ];
      note = "pyproject.toml uses OpenGL windowing packages";
    }
    {
      dependencies = [
        "opencv-python"
        "opencv-contrib-python"
      ];
      components = [
        "desktop-gl"
        "media"
      ];
      requires = [
        "runtime.graphics.opengl"
        "runtime.media.ffmpeg"
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
        "torchcodec"
      ];
      components = ["media"];
      requires = ["runtime.media.ffmpeg"];
      note = "pyproject.toml uses FFmpeg/media packages";
    }
    {
      dependencies = ["lerobot"];
      components = [
        "media"
        "desktop-gl"
      ];
      requires = [
        "runtime.graphics.opengl"
        "runtime.media.ffmpeg"
      ];
      note = "LeRobot workflows commonly need media and graphics runtime libraries";
    }
    {
      dependencies = ["torchvision"];
      components = ["media"];
      requires = ["runtime.media.ffmpeg"];
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
        "desktop-gl"
      ];
      requires = [
        "runtime.graphics.opengl"
        "runtime.ui.qt6"
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
      requires = [
        "runtime.native.compiler"
        "runtime.native.libstdcxx"
      ];
      note = "ML packages often build or load native extensions";
    }
    {
      dependencies = [
        "cuda-python"
        "cupy"
        "cupy-cuda11x"
        "cupy-cuda12x"
        "deepspeed"
        "flash-attn"
        "nvidia-curobo"
        "pytorch3d"
        "torch3d"
      ];
      components = ["cuda-toolkit"];
      requires = [
        "runtime.cuda.headers"
        "runtime.cuda.link.cudart"
        "runtime.cuda.nvcc"
      ];
      note = "CUDA extension packages need CUDA compiler, headers, and link support";
    }
    {
      dependencies = ["isaacsim"];
      components = [
        "isaac-sim"
        "desktop-gl"
        "host-nvidia-gl"
      ];
      requires = [
        "host.cuda.driver"
        "host.cuda.libcuda"
        "host.graphics.nvidia"
        "runtime.graphics.egl"
        "runtime.graphics.opengl"
        "runtime.sim.isaac"
      ];
      note = "Isaac Sim Python wheels need host NVIDIA CUDA and graphics runtime support";
    }
    {
      dependencies = ["flash-attn"];
      components = ["native-build"];
      requires = [
        "runtime.cuda.nvcc"
        "runtime.native.compiler"
      ];
      note = "FlashAttention builds CUDA native extensions";
    }
    {
      dependencies = ["evdev"];
      components = [
        "linux-headers"
        "native-build"
      ];
      requires = [
        "runtime.native.compiler"
        "runtime.native.linux-headers"
      ];
      note = "evdev native extensions include Linux input headers";
    }
  ];

  compoundDependencyRules = [
    {
      dependenciesAll = [
        ["matplotlib"]
        [
          "pyside6"
          "pyqt6"
          "pyqt5"
        ]
      ];
      components = ["matplotlib-qt"];
      requires = ["runtime.ui.matplotlib-qt"];
      note = "pyproject.toml uses Matplotlib with Qt bindings";
    }
  ];
}
