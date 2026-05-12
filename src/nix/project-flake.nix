{
  nixpkgs,
  nixpkgs-python,
  nixgl ? null,
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
      hostGraphics = spec.hostGraphics or null;
      ccRuntimeLib = lib.getLib pkgs.stdenv.cc.cc;
      libcDev = pkgs.stdenv.cc.libc_dev;
      legacyCryptRuntimeLib = pkgs.libxcrypt-legacy;
      zlibRuntimeLib = lib.getLib pkgs.zlib;
      roboUv = let
        uvWrapper = pkgs.writeShellScriptBin "uv" ''
          real_uv="${pkgs.uv}/bin/uv"

          if [ -n "''${UV_PROJECT_ENVIRONMENT:-}" ]; then
            export VIRTUAL_ENV="$UV_PROJECT_ENVIRONMENT"
            if [ -d "$UV_PROJECT_ENVIRONMENT/bin" ]; then
              case ":$PATH:" in
                *":$UV_PROJECT_ENVIRONMENT/bin:"*) ;;
                *) export PATH="$UV_PROJECT_ENVIRONMENT/bin:$PATH" ;;
              esac
            fi
          fi

          if [ "''${1:-}" = "pip" ] && [ "''${2:-}" = "install" ] && [ -n "''${UV_PROJECT_ENVIRONMENT:-}" ] && [ -x "$UV_PROJECT_ENVIRONMENT/bin/python" ]; then
            robo_uv_has_target=0
            for robo_uv_arg in "$@"; do
              case "$robo_uv_arg" in
                --)
                  break
                  ;;
                --python|--python=*|-p|--system|--active|--target|--target=*|--prefix|--prefix=*)
                  robo_uv_has_target=1
                  ;;
              esac
            done

            if [ "$robo_uv_has_target" = 0 ]; then
              shift 2
              exec "$real_uv" pip install --python "$UV_PROJECT_ENVIRONMENT/bin/python" "$@"
            fi
          fi

          exec "$real_uv" "$@"
        '';
      in
        pkgs.symlinkJoin {
          name = "robo-uv";
          paths = [pkgs.uv];
          postBuild = ''
            rm -f $out/bin/uv
            ln -sf ${uvWrapper}/bin/uv $out/bin/uv
          '';
        };
      nativeBuildCmake = let
        cmakeWrapper = pkgs.writeShellScriptBin "cmake" ''
          real_cmake="${pkgs.cmake}/bin/cmake"
          robo_cmake_configure=1

          for robo_cmake_arg in "$@"; do
            case "$robo_cmake_arg" in
              --build|--install|--open|--find-package|-E|-P|--version|-version|/version|--help|-help|/help)
                robo_cmake_configure=0
                ;;
              --help-*)
                robo_cmake_configure=0
                ;;
            esac
          done

          if [ "$robo_cmake_configure" != 1 ]; then
            exec "$real_cmake" "$@"
          fi

          robo_cmake_stdout="$(${pkgs.coreutils}/bin/mktemp "''${TMPDIR:-/tmp}/robo-cmake-stdout.XXXXXX")" || exec "$real_cmake" "$@"
          robo_cmake_stderr="$(${pkgs.coreutils}/bin/mktemp "''${TMPDIR:-/tmp}/robo-cmake-stderr.XXXXXX")" || {
            rm -f "$robo_cmake_stdout"
            exec "$real_cmake" "$@"
          }
          trap 'rm -f "$robo_cmake_stdout" "$robo_cmake_stderr"' EXIT HUP INT TERM

          "$real_cmake" "$@" >"$robo_cmake_stdout" 2>"$robo_cmake_stderr"
          robo_cmake_status=$?
          ${pkgs.coreutils}/bin/cat "$robo_cmake_stdout"
          ${pkgs.coreutils}/bin/cat "$robo_cmake_stderr" >&2

          if [ "$robo_cmake_status" -ne 0 ] && ${pkgs.gnugrep}/bin/grep -q "Could not find a package configuration file provided by" "$robo_cmake_stderr"; then
            robo_cmake_package="$(
              ${pkgs.gnused}/bin/sed -n 's/.*provided by "\([^"]*\)".*/\1/p' "$robo_cmake_stderr" | ${pkgs.coreutils}/bin/head -n 1
            )"
            if [ -n "$robo_cmake_package" ]; then
              printf '%s\n' "robo-nix hint: CMake could not find package '$robo_cmake_package'." >&2
              printf '%s\n' "robo-nix hint: native-build supplies compiler tools and common native runtime libraries; package-specific CMake config files must come from the project, the uv build environment, or explicit robo.nix additions." >&2
              if [ "$robo_cmake_package" = "Qt6" ]; then
                printf '%s\n' "robo-nix hint: add \"qt6\" to components in robo.nix for Qt6 CMake packages and runtime libraries." >&2
              fi
              printf '%s\n' "robo-nix hint: patch the package build to set ''${robo_cmake_package}_DIR or CMAKE_PREFIX_PATH to the prefix containing ''${robo_cmake_package}Config.cmake." >&2
            fi
          fi

          if [ "$robo_cmake_status" -ne 0 ] && ${pkgs.gnugrep}/bin/grep -q "is not a full path to an existing compiler tool" "$robo_cmake_stderr" "$robo_cmake_stdout"; then
            printf '%s\n' "robo-nix hint: CMake is using a cached compiler path that no longer exists." >&2
            printf '%s\n' "robo-nix hint: remove the affected CMake build directory or CMakeCache.txt, then rerun inside the current runtime shell." >&2
          fi

          exit "$robo_cmake_status"
        '';
      in
        pkgs.symlinkJoin {
          name = "robo-native-build-cmake";
          paths = [pkgs.cmake];
          postBuild = ''
            rm -f $out/bin/cmake
            ln -sf ${cmakeWrapper}/bin/cmake $out/bin/cmake
          '';
        };
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
        python-uv = [python roboUv];
        native-build = [nativeBuildCmake pkgs.pkg-config pkgs.stdenv.cc];
        linux-headers = [pkgs.linuxHeaders];
        desktop-gl = [
          pkgs.dbus
          pkgs.fontconfig
          pkgs.glib
          pkgs.libdrm
          pkgs.libgbm
          pkgs.libGL
          pkgs.libGLU
          pkgs.libglvnd
          pkgs.mesa
          pkgs.vulkan-loader
          pkgs.wayland
          pkgs.libice
          pkgs.libsm
          pkgs.libx11
          pkgs.libxau
          pkgs.libxcb
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
          pkgs.xorg.libxshmfence
          pkgs.libXt
          pkgs.libxtst
        ];
        qt6 = [
          pkgs.qt6.qtbase
          pkgs.qt6.qt5compat
        ];
        cuda-toolkit = [
          cudaPackages.backendStdenv.cc
          cudaToolkit
        ];
      };

      componentRuntimeLibraries = {
        python-uv = [python];
        native-build = [ccRuntimeLib legacyCryptRuntimeLib zlibRuntimeLib];
        linux-headers = [];
        desktop-gl = componentPackages.desktop-gl;
        qt6 = componentPackages.qt6;
        cuda-toolkit = [
          cudaPackages.cuda_cudart
          cudaPackages.cuda_nvrtc
          cudaPackages.libnpp.lib
        ];
      };

      unknownComponents = lib.filter (component: !(builtins.hasAttr component componentPackages)) selectedComponents;
      validHostGraphics = [null "nvidia" "nixgl" "nixgl-nvidia"];
      hasComponent = component: builtins.elem component selectedComponents;
      componentPackageLists = map (component: builtins.getAttr component componentPackages) selectedComponents;
      componentRuntimeLibraryLists = map (component: builtins.getAttr component componentRuntimeLibraries) selectedComponents;
      runtimeLibraries = (builtins.concatLists componentRuntimeLibraryLists) ++ extraRuntimeLibraries pkgs;
      runtimeLibraryPath = lib.makeLibraryPath runtimeLibraries;
      nixglPackages =
        if nixgl == null
        then {}
        else nixgl.packages.${system};
      bundledNixglWrapper =
        if hostGraphics == "nixgl-nvidia" && builtins.hasAttr "nixGLNvidia" nixglPackages
        then "${nixglPackages.nixGLNvidia}/bin/nixGLNvidia"
        else if hostGraphics == "nixgl" && builtins.hasAttr "nixGLDefault" nixglPackages
        then "${nixglPackages.nixGLDefault}/bin/nixGL"
        else "";
    in {
      default =
        if unknownComponents != []
        then throw "robo-nix: unknown components in robo.nix: ${lib.concatStringsSep ", " unknownComponents}"
        else if !(builtins.elem hostGraphics validHostGraphics)
        then throw "robo-nix: hostGraphics in robo.nix must be null, \"nvidia\", \"nixgl\", or \"nixgl-nvidia\""
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
                export ROBO_NIX_HOST_GRAPHICS="${if hostGraphics == null then "none" else hostGraphics}"
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
              + lib.optionalString (hasComponent "native-build") ''

                export ROBO_NIX_LIBC_DEV="${libcDev}"
              ''
              + lib.optionalString (hasComponent "desktop-gl") ''

                export __EGL_VENDOR_LIBRARY_FILENAMES="''${__EGL_VENDOR_LIBRARY_FILENAMES:-${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json}"
              ''
              + lib.optionalString (hostGraphics == "nvidia") ''

                robo_nix_select_host_manifest() {
                  local override="$1"
                  shift
                  if [ -n "$override" ]; then
                    printf '%s\n' "$override"
                    return
                  fi

                  local fallback="$1"
                  for candidate in "$@"; do
                    if [ -e "$candidate" ]; then
                      printf '%s\n' "$candidate"
                      return
                    fi
                  done
                  printf '%s\n' "$fallback"
                }

                robo_nix_nvidia_vk_icd="$(robo_nix_select_host_manifest "''${ROBO_NIX_NVIDIA_VK_ICD:-}" \
                  /run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json \
                  /usr/share/vulkan/icd.d/nvidia_icd.json)"
                robo_nix_nvidia_egl_vendor="$(robo_nix_select_host_manifest "''${ROBO_NIX_NVIDIA_EGL_VENDOR:-}" \
                  /run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json \
                  /usr/share/glvnd/egl_vendor.d/10_nvidia.json)"

                export VK_ICD_FILENAMES="$robo_nix_nvidia_vk_icd"
                export VK_DRIVER_FILES="$VK_ICD_FILENAMES"
                export __EGL_VENDOR_LIBRARY_FILENAMES="$robo_nix_nvidia_egl_vendor"
                export __NV_PRIME_RENDER_OFFLOAD=1
                export __GLX_VENDOR_LIBRARY_NAME=nvidia
                export __VK_LAYER_NV_optimus=NVIDIA_only
                unset -f robo_nix_select_host_manifest
                unset robo_nix_nvidia_vk_icd robo_nix_nvidia_egl_vendor
              ''
              + lib.optionalString (hostGraphics == "nixgl" || hostGraphics == "nixgl-nvidia") ''

                robo_nix_nixgl="''${ROBO_NIX_NIXGL:-}"
                if [ -z "$robo_nix_nixgl" ] && [ -n "${bundledNixglWrapper}" ] && [ -x "${bundledNixglWrapper}" ]; then
                  robo_nix_nixgl="${bundledNixglWrapper}"
                fi
                if [ -z "$robo_nix_nixgl" ]; then
                  for robo_nix_nixgl_candidate in ${if hostGraphics == "nixgl-nvidia" then "nixGLNvidia" else "nixGLNvidia nixGL nixGLMesa"}; do
                    if command -v "$robo_nix_nixgl_candidate" >/dev/null 2>&1; then
                      robo_nix_nixgl="$(command -v "$robo_nix_nixgl_candidate")"
                      break
                    fi
                  done
                fi

                if [ -z "$robo_nix_nixgl" ] || [ ! -x "$robo_nix_nixgl" ]; then
                  printf '%s\n' "robo-nix: hostGraphics = \"${hostGraphics}\" requires ${if hostGraphics == "nixgl-nvidia" then "nixGLNvidia" else "nixGL, nixGLNvidia, or nixGLMesa"} on PATH." >&2
                  printf '%s\n' "robo-nix: set ROBO_NIX_NIXGL to the nixGL wrapper path for uncommon layouts." >&2
                  return 1 2>/dev/null || exit 1
                fi

                robo_nix_runtime_ld_library_path="''${LD_LIBRARY_PATH:-}"
                unset LIBGL_DRIVERS_PATH LIBVA_DRIVERS_PATH GBM_BACKENDS_PATH
                unset __EGL_VENDOR_LIBRARY_FILENAMES __GLX_VENDOR_LIBRARY_NAME
                unset __NV_PRIME_RENDER_OFFLOAD __VK_LAYER_NV_optimus
                unset VK_ICD_FILENAMES VK_DRIVER_FILES VK_LAYER_PATH

                while IFS= read -r -d "" robo_nix_nixgl_entry; do
                  case "$robo_nix_nixgl_entry" in
                    LD_LIBRARY_PATH=*)
                      robo_nix_nixgl_ld_library_path="''${robo_nix_nixgl_entry#LD_LIBRARY_PATH=}"
                      if [ -n "$robo_nix_nixgl_ld_library_path" ] && [ -n "$robo_nix_runtime_ld_library_path" ]; then
                        export LD_LIBRARY_PATH="$robo_nix_nixgl_ld_library_path:$robo_nix_runtime_ld_library_path"
                      elif [ -n "$robo_nix_nixgl_ld_library_path" ]; then
                        export LD_LIBRARY_PATH="$robo_nix_nixgl_ld_library_path"
                      fi
                      ;;
                    LIBGL_DRIVERS_PATH=*|LIBVA_DRIVERS_PATH=*|GBM_BACKENDS_PATH=*|__EGL_VENDOR_LIBRARY_FILENAMES=*|__GLX_VENDOR_LIBRARY_NAME=*|__NV_PRIME_RENDER_OFFLOAD=*|__VK_LAYER_NV_optimus=*|VK_ICD_FILENAMES=*|VK_DRIVER_FILES=*|VK_LAYER_PATH=*)
                      export "$robo_nix_nixgl_entry"
                      ;;
                  esac
                done < <(env \
                  -u LD_LIBRARY_PATH \
                  -u LIBGL_DRIVERS_PATH \
                  -u LIBVA_DRIVERS_PATH \
                  -u GBM_BACKENDS_PATH \
                  -u __EGL_VENDOR_LIBRARY_FILENAMES \
                  -u __GLX_VENDOR_LIBRARY_NAME \
                  -u __NV_PRIME_RENDER_OFFLOAD \
                  -u __VK_LAYER_NV_optimus \
                  -u VK_ICD_FILENAMES \
                  -u VK_DRIVER_FILES \
                  -u VK_LAYER_PATH \
                  "$robo_nix_nixgl" env -0)

                unset robo_nix_nixgl robo_nix_nixgl_candidate robo_nix_nixgl_entry
                unset robo_nix_nixgl_ld_library_path robo_nix_runtime_ld_library_path
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
