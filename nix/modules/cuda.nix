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
        cuda_cccl.dev
        cuda_cccl
        cuda_cudart.dev
        cuda_cudart.lib
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
      + common.prependPath "CPATH" "$CUDA_PATH/include"
      + "\n"
      + common.prependPath "C_INCLUDE_PATH" "$CUDA_PATH/include"
      + "\n"
      + common.prependPath "CPLUS_INCLUDE_PATH" "$CUDA_PATH/include"
      + "\n"
      + common.prependPath "CMAKE_INCLUDE_PATH" "$CUDA_PATH/include"
      + "\n"
      + common.prependPath "CMAKE_LIBRARY_PATH" "$CUDA_PATH/lib"
      + "\n"
      + common.prependPath "LIBRARY_PATH" "$CUDA_PATH/lib"
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "$CUDA_PATH/lib";
    supportedSystems = common.x86LinuxSystems;
    gpuRequired = true;
    check = common.mkComponentCheck "cuda-toolkit" [];
    diagnostics = ''
      if [ -z "''${CUDA_PATH:-}" ] || [ ! -d "$CUDA_PATH" ]; then
        check_error "CUDA native build toolkit is not visible"
        check_hint "cuda-toolkit should set CUDA_HOME/CUDA_PATH inside the runtime"
      else
        cuda_missing=0
        if [ ! -x "$CUDA_PATH/bin/nvcc" ]; then
          check_error "CUDA native compiler is missing: $CUDA_PATH/bin/nvcc"
          cuda_missing=1
        fi
        if [ ! -f "$CUDA_PATH/include/cuda_runtime.h" ]; then
          check_error "CUDA runtime headers are missing: $CUDA_PATH/include/cuda_runtime.h"
          cuda_missing=1
        fi
        if [ ! -f "$CUDA_PATH/include/nv/target" ]; then
          check_error "CUDA CCCL headers are missing: $CUDA_PATH/include/nv/target"
          cuda_missing=1
        fi
        if [ ! -e "$CUDA_PATH/lib/libcudart.so" ]; then
          check_error "CUDA runtime link library is missing: $CUDA_PATH/lib/libcudart.so"
          cuda_missing=1
        fi
        if [ "$cuda_missing" -eq 0 ]; then
          check_ok "CUDA native build surface present (nvcc, headers, libcudart)"
        fi
      fi
    '';
  };
}
