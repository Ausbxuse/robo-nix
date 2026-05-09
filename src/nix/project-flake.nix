{
  nixpkgs,
  nixpkgs-python,
}: let
  systems = ["x86_64-linux" "aarch64-linux"];

  mkProjectFlakeFromManifest = manifestPath:
    mkProjectFlake {
      projectRoot = builtins.dirOf manifestPath;
      spec = import manifestPath;
    };

  mkProjectFlake = {
    projectRoot ? ./.,
    spec,
  }: let
    forAllSystems = nixpkgs.lib.genAttrs systems;
  in {
    devShells = forAllSystems (system: let
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };
      lib = pkgs.lib;

      rawPythonVersion = lib.strings.removeSuffix "\n" (builtins.readFile (projectRoot + "/.python-version"));
      pythonVersionParts = lib.splitString "." rawPythonVersion;
      pythonMajorMinor = lib.concatStringsSep "." (lib.take 2 pythonVersionParts);
      pythonPackages = nixpkgs-python.packages.${system};
      python =
        if builtins.hasAttr rawPythonVersion pythonPackages
        then builtins.getAttr rawPythonVersion pythonPackages
        else if builtins.hasAttr pythonMajorMinor pythonPackages
        then builtins.getAttr pythonMajorMinor pythonPackages
        else throw "robo-nix: nixpkgs-python does not provide Python ${rawPythonVersion} for ${system}";

      selectedComponents = spec.components or [];
      extraPackages = spec.extraPackages or (_: []);
      extraRuntimeLibraries = spec.extraRuntimeLibraries or (_: []);
      ccRuntimeLib = lib.getLib pkgs.stdenv.cc.cc;
      zlibRuntimeLib = lib.getLib pkgs.zlib;
      cudaPackages = pkgs.cudaPackages;
      cudaToolkit = pkgs.symlinkJoin {
        name = "robo-cuda-toolkit-${cudaPackages.cudaMajorMinorVersion}";
        paths = [
          cudaPackages.cuda_cccl
          cudaPackages.cuda_cudart
          cudaPackages.cuda_nvcc
          cudaPackages.cuda_nvrtc
          cudaPackages.cuda_profiler_api
          cudaPackages.libnpp.lib
        ];
      };

      componentPackages = {
        python-uv = [python pkgs.uv];
        native-build = [pkgs.cmake pkgs.pkg-config pkgs.stdenv.cc];
        linux-headers = [pkgs.linuxHeaders];
        desktop-gl = [
          pkgs.dbus
          pkgs.fontconfig
          pkgs.glib
          pkgs.libGL
          pkgs.libglvnd
          pkgs.mesa
          pkgs.vulkan-loader
          pkgs.wayland
          pkgs.libice
          pkgs.libsm
          pkgs.libx11
          pkgs.libxau
          pkgs.libxcomposite
          pkgs.libxcursor
          pkgs.libxdamage
          pkgs.libxdmcp
          pkgs.libxext
          pkgs.libxfixes
          pkgs.libxi
          pkgs.libxkbcommon
          pkgs.libxrandr
          pkgs.libxrender
          pkgs.libxtst
        ];
        cuda-toolkit = [
          cudaPackages.backendStdenv.cc
          cudaToolkit
        ];
      };

      componentRuntimeLibraries = {
        python-uv = [];
        native-build = [ccRuntimeLib zlibRuntimeLib];
        linux-headers = [];
        desktop-gl = componentPackages.desktop-gl;
        cuda-toolkit = [
          cudaPackages.cuda_cudart
          cudaPackages.cuda_nvrtc
          cudaPackages.libnpp.lib
        ];
      };

      unknownComponents = lib.filter (component: !(builtins.hasAttr component componentPackages)) selectedComponents;
      hasComponent = component: builtins.elem component selectedComponents;
      componentPackageLists = map (component: builtins.getAttr component componentPackages) selectedComponents;
      componentRuntimeLibraryLists = map (component: builtins.getAttr component componentRuntimeLibraries) selectedComponents;
      runtimeLibraries = (builtins.concatLists componentRuntimeLibraryLists) ++ extraRuntimeLibraries pkgs;
      runtimeLibraryPath = lib.makeLibraryPath runtimeLibraries;
    in {
      default =
        if unknownComponents != []
        then throw "robo-nix: unknown components in robo.nix: ${lib.concatStringsSep ", " unknownComponents}"
        else
          pkgs.mkShell {
            packages = (builtins.concatLists componentPackageLists) ++ extraPackages pkgs;

            shellHook =
              ''
                export ROBO_NIX_PYTHON="${python}/bin/python"
                export UV_PYTHON="$ROBO_NIX_PYTHON"
                export UV_PYTHON_DOWNLOADS=never
                export UV_PROJECT_ENVIRONMENT="''${UV_PROJECT_ENVIRONMENT:-$PWD/.venv}"
                export UV_CACHE_DIR="''${UV_CACHE_DIR:-$PWD/.robo-nix/uv-cache}"
                export ROBO_NIX_COMPONENTS="${lib.concatStringsSep ":" selectedComponents}"
                unset PYTHONHOME
                unset PYTHONPATH

                if [ -d "$UV_PROJECT_ENVIRONMENT/bin" ]; then
                  export VIRTUAL_ENV="$UV_PROJECT_ENVIRONMENT"
                  case ":$PATH:" in
                    *":$UV_PROJECT_ENVIRONMENT/bin:"*) ;;
                    *) export PATH="$UV_PROJECT_ENVIRONMENT/bin:$PATH" ;;
                  esac
                fi
              ''
              + lib.optionalString (runtimeLibraryPath != "") ''

                case ":''${LD_LIBRARY_PATH:-}:" in
                  *":${runtimeLibraryPath}:"*) ;;
                  *) export LD_LIBRARY_PATH="${runtimeLibraryPath}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
                esac
              ''
              + lib.optionalString (hasComponent "desktop-gl") ''

                export __EGL_VENDOR_LIBRARY_FILENAMES="''${__EGL_VENDOR_LIBRARY_FILENAMES:-${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json}"
              ''
              + lib.optionalString (hasComponent "linux-headers") ''

                export ROBO_NIX_LINUX_HEADERS="${pkgs.linuxHeaders}/include"
                case ":''${CPATH:-}:" in
                  *":$ROBO_NIX_LINUX_HEADERS:"*) ;;
                  *) export CPATH="$ROBO_NIX_LINUX_HEADERS''${CPATH:+:$CPATH}" ;;
                esac
                case ":''${C_INCLUDE_PATH:-}:" in
                  *":$ROBO_NIX_LINUX_HEADERS:"*) ;;
                  *) export C_INCLUDE_PATH="$ROBO_NIX_LINUX_HEADERS''${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}" ;;
                esac
              ''
              + lib.optionalString (hasComponent "cuda-toolkit") ''

                export CUDA_PATH="''${ROBO_NIX_CUDA_ROOT:-${cudaToolkit}}"
                export CUDA_HOME="$CUDA_PATH"
                export CUDA_TOOLKIT_ROOT_DIR="$CUDA_PATH"
                export CUDAToolkit_ROOT="$CUDA_PATH"
                export CUDAHOSTCXX="${cudaPackages.backendStdenv.cc}/bin/c++"
                export CC="${cudaPackages.backendStdenv.cc}/bin/cc"
                export CXX="${cudaPackages.backendStdenv.cc}/bin/c++"
                export NVIDIA_VISIBLE_DEVICES="''${NVIDIA_VISIBLE_DEVICES:-all}"

                case ":$PATH:" in
                  *":$CUDA_PATH/bin:"*) ;;
                  *) export PATH="$CUDA_PATH/bin:$PATH" ;;
                esac
                case ":''${CPATH:-}:" in
                  *":$CUDA_PATH/include:"*) ;;
                  *) export CPATH="$CUDA_PATH/include''${CPATH:+:$CPATH}" ;;
                esac
                case ":''${LIBRARY_PATH:-}:" in
                  *":$CUDA_PATH/lib:"*) ;;
                  *) export LIBRARY_PATH="$CUDA_PATH/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}" ;;
                esac
                case ":''${LD_LIBRARY_PATH:-}:" in
                  *":$CUDA_PATH/lib:"*) ;;
                  *) export LD_LIBRARY_PATH="$CUDA_PATH/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
                esac
              ''
              + ''

                if [ -n "''${ROBO_NIX_LIBCUDA_PATH:-}" ]; then
                  robo_nix_cuda_driver_dir=""
                  if [ -f "$ROBO_NIX_LIBCUDA_PATH" ]; then
                    robo_nix_cuda_driver_dir="$(dirname "$ROBO_NIX_LIBCUDA_PATH")"
                  elif [ -d "$ROBO_NIX_LIBCUDA_PATH" ] && [ -e "$ROBO_NIX_LIBCUDA_PATH/libcuda.so.1" ]; then
                    robo_nix_cuda_driver_dir="$ROBO_NIX_LIBCUDA_PATH"
                    export ROBO_NIX_LIBCUDA_PATH="$ROBO_NIX_LIBCUDA_PATH/libcuda.so.1"
                  fi

                  if [ -n "$robo_nix_cuda_driver_dir" ]; then
                    export TRITON_LIBCUDA_PATH="''${TRITON_LIBCUDA_PATH:-$robo_nix_cuda_driver_dir}"
                    if [ -n "''${ROBO_NIX_HOST_LIBCUDA_BRIDGE:-}" ]; then
                      case ":''${LD_LIBRARY_PATH:-}:" in
                        *":$ROBO_NIX_HOST_LIBCUDA_BRIDGE:"*) ;;
                        *) export LD_LIBRARY_PATH="$ROBO_NIX_HOST_LIBCUDA_BRIDGE''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
                      esac
                    else
                      case ":''${LD_LIBRARY_PATH:-}:" in
                        *":$robo_nix_cuda_driver_dir:"*) ;;
                        *) export LD_LIBRARY_PATH="$robo_nix_cuda_driver_dir''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
                      esac
                    fi
                  fi
                  unset robo_nix_cuda_driver_dir
                fi
              ''
              + (spec.shellHook or "");
          };
    });
  };
in {
  inherit mkProjectFlake mkProjectFlakeFromManifest;
}
