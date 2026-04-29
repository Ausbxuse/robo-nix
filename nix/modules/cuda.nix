{common}: {
  cuda-toolkit = {
    envSpec,
    pkgs,
    ...
  }: let
    requestedCuda = envSpec.cudaWheelVersion or null;
    cudaPackages =
      if requestedCuda == null
      then pkgs.cudaPackages
      else let
        attr = "cudaPackages_" + builtins.replaceStrings ["."] ["_"] requestedCuda;
      in
        if builtins.hasAttr attr pkgs
        then builtins.getAttr attr pkgs
        else throw "robo-nix: cuda-toolkit requested CUDA ${requestedCuda}, but nixpkgs does not provide ${attr}. Align cudaWheelVersion, uv.lock CUDA wheels, or nixpkgs.";
    cudaCompiler = cudaPackages.backendStdenv.cc;
    cudaToolkit = pkgs.symlinkJoin {
      name = "robo-cuda-toolkit-${cudaPackages.cudaMajorMinorVersion}";
      paths = with cudaPackages; [
        cuda_cccl
        cuda_cudart
        cuda_nvcc
        cuda_nvrtc
        cuda_profiler_api
      ];
    };
  in {
    packages = [
      cudaCompiler
      cudaToolkit
    ];
    shellInit =
      common.exportDefaults {
        CUDA_PATH = "\${ROBO_NIX_CUDA_ROOT:-${cudaToolkit}}";
        NVIDIA_VISIBLE_DEVICES = "all";
      }
      + "\n"
      + common.prependPath "PATH" "$CUDA_PATH/bin"
      + "\n"
      + common.prependPath "PATH" "${cudaCompiler}/bin"
      + "\n"
      + common.exportVars {
        CC = "${cudaCompiler}/bin/cc";
        CUDA_HOME = "$CUDA_PATH";
        CUDAHOSTCXX = "${cudaCompiler}/bin/c++";
        CUDA_TOOLKIT_ROOT_DIR = "$CUDA_PATH";
        CUDAToolkit_ROOT = "$CUDA_PATH";
        CXX = "${cudaCompiler}/bin/c++";
      }
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "$CUDA_PATH/lib"
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "/run/opengl-driver/lib";
    supportedSystems = common.x86LinuxSystems;
    gpuRequired = true;
    check = common.mkComponentCheck "cuda-toolkit" [];
  };
}
