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
      hostGraphics = spec.hostGraphics or "auto";
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
          pkgs.libxshmfence
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
      validHostGraphics = [null "auto" "nixgl" "nixgl-nvidia"];
      hasComponent = component: builtins.elem component selectedComponents;
      componentPackageLists = map (component: builtins.getAttr component componentPackages) selectedComponents;
      componentRuntimeLibraryLists = map (component: builtins.getAttr component componentRuntimeLibraries) selectedComponents;
      runtimeLibraries = (builtins.concatLists componentRuntimeLibraryLists) ++ extraRuntimeLibraries pkgs;
      runtimeLibraryPath = lib.makeLibraryPath runtimeLibraries;
      nixglPackages =
        if nixgl == null
        then {}
        else nixgl.packages.${system};
      nixglSource =
        if nixgl == null
        then ""
        else nixgl.outPath;
      nixglNvidiaSource =
        if nixgl == null || hostGraphics != "nixgl-nvidia"
        then ""
        else
          pkgs.runCommand "robo-nixgl-nvidia-source" {} ''
            cp -R --no-preserve=mode ${nixglSource}/. "$out"
            if ! grep -Fq '        kernel = null;' "$out/nixGL.nix"; then
              printf '%s\n' "robo-nix: expected nixGL NVIDIA compatibility patch target missing" >&2
              exit 1
            fi
            substituteInPlace "$out/nixGL.nix" --replace '        kernel = null;' ""
          '';
      nixglNvidiaPkgsArg = ''import ${nixpkgs} { system = "${system}"; config.allowUnfree = true; }'';
      bundledNixglWrapper =
        if (hostGraphics == "auto" || hostGraphics == "nixgl") && builtins.hasAttr "nixGLDefault" nixglPackages
        then "${nixglPackages.nixGLDefault}/bin/nixGL"
        else "";
      graphicsWrapperEnvNames = [
        "LIBGL_DRIVERS_PATH"
        "LIBVA_DRIVERS_PATH"
        "GBM_BACKENDS_PATH"
        "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS"
        "__EGL_VENDOR_LIBRARY_FILENAMES"
        "__GLX_VENDOR_LIBRARY_NAME"
        "__NV_PRIME_RENDER_OFFLOAD"
        "__VK_LAYER_NV_optimus"
        "VK_ICD_FILENAMES"
        "VK_DRIVER_FILES"
        "VK_LAYER_PATH"
      ];
      graphicsWrapperCasePattern =
        lib.concatStringsSep "|" (map (name: "${name}=*") graphicsWrapperEnvNames);
      graphicsWrapperUnset = lib.concatStringsSep " " graphicsWrapperEnvNames;
      graphicsWrapperEnvScrubArgs =
        lib.concatStringsSep " \\\n                    " (map (name: "-u ${name}") (["LD_LIBRARY_PATH"] ++ graphicsWrapperEnvNames));
    in {
      default =
        if unknownComponents != []
        then throw "robo-nix: unknown components in robo.nix: ${lib.concatStringsSep ", " unknownComponents}"
        else if !(builtins.elem hostGraphics validHostGraphics)
        then throw "robo-nix: hostGraphics in robo.nix must be null, \"auto\", \"nixgl\", or \"nixgl-nvidia\""
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
                # mkShell runs shellHook under Bash. Keep colon-list updates in
                # one place so CUDA, graphics, headers, and venv setup have the
                # same duplicate-prevention behavior.
                robo_nix_prepend_path() {
                  local robo_nix_prepend_name="$1"
                  local robo_nix_prepend_value="$2"
                  local robo_nix_prepend_current="''${!robo_nix_prepend_name:-}"
                  case ":$robo_nix_prepend_current:" in
                    *":$robo_nix_prepend_value:"*) ;;
                    *)
                      if [ -n "$robo_nix_prepend_current" ]; then
                        export "$robo_nix_prepend_name=$robo_nix_prepend_value:$robo_nix_prepend_current"
                      else
                        export "$robo_nix_prepend_name=$robo_nix_prepend_value"
                      fi
                      ;;
                  esac
                }
                robo_nix_host_graphics_policy="${
                  if hostGraphics == null
                  then "none"
                  else hostGraphics
                }"
                if [ "$robo_nix_host_graphics_policy" = "auto" ]; then
                  if [ -d /run/opengl-driver/lib ]; then
                    robo_nix_host_graphics_policy=nixos
                  else
                    robo_nix_host_graphics_policy=nixgl
                  fi
                fi
                export ROBO_NIX_HOST_GRAPHICS="$robo_nix_host_graphics_policy"
                unset PYTHONHOME
                unset PYTHONPATH

                if [ -d "$UV_PROJECT_ENVIRONMENT/bin" ]; then
                  export VIRTUAL_ENV="$UV_PROJECT_ENVIRONMENT"
                  robo_nix_prepend_path PATH "$UV_PROJECT_ENVIRONMENT/bin"
                fi
              ''
              + lib.optionalString (runtimeLibraryPath != "") ''

                robo_nix_prepend_path LD_LIBRARY_PATH "${runtimeLibraryPath}"
              ''
              + lib.optionalString (hasComponent "native-build") ''

                export ROBO_NIX_LIBC_DEV="${libcDev}"
              ''
              + lib.optionalString (hasComponent "desktop-gl") ''

                export __EGL_VENDOR_LIBRARY_FILENAMES="''${__EGL_VENDOR_LIBRARY_FILENAMES:-${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json}"
              ''
              + lib.optionalString (hostGraphics != null) ''

                if [ "$robo_nix_host_graphics_policy" = "nixos" ] && [ -d /run/opengl-driver/lib ]; then
                  robo_nix_prepend_path LD_LIBRARY_PATH "/run/opengl-driver/lib"
                fi

                if [ "$robo_nix_host_graphics_policy" = "nixgl" ] || [ "$robo_nix_host_graphics_policy" = "nixgl-nvidia" ]; then
                  # robo imports only the runtime variables selected by nixGL.
                  # The launched command still runs under robo so shell refresh,
                  # caching, and `robo run` stay on one runtime path.
                  robo_nix_nixgl="''${ROBO_NIX_NIXGL:-}"
                  if [ -z "$robo_nix_nixgl" ] && [ "$robo_nix_host_graphics_policy" != "nixgl-nvidia" ] && [ -n "${bundledNixglWrapper}" ] && [ -x "${bundledNixglWrapper}" ]; then
                    robo_nix_nixgl="${bundledNixglWrapper}"
                  fi
                  if [ -z "$robo_nix_nixgl" ] && [ "$robo_nix_host_graphics_policy" = "nixgl-nvidia" ] && [ -n "${nixglNvidiaSource}" ]; then
                    robo_nix_nvidia_version="''${ROBO_NIX_NVIDIA_VERSION:-}"
                    if [ -z "$robo_nix_nvidia_version" ]; then
                      for robo_nix_nvidia_smi in "$(command -v nvidia-smi 2>/dev/null || true)" /usr/bin/nvidia-smi /run/current-system/sw/bin/nvidia-smi; do
                        if [ -n "$robo_nix_nvidia_smi" ] && [ -x "$robo_nix_nvidia_smi" ]; then
                          robo_nix_nvidia_version="$("$robo_nix_nvidia_smi" --query-gpu=driver_version --format=csv,noheader 2>/dev/null | sed -n '1p' | tr -d '[:space:]')"
                          if [ -n "$robo_nix_nvidia_version" ]; then
                            break
                          fi
                        fi
                      done
                    fi
                    if [ -z "$robo_nix_nvidia_version" ] && [ -r /proc/driver/nvidia/version ]; then
                      robo_nix_nvidia_version="$(sed -n 's/.*Module  *\([0-9.][0-9.]*\).*/\1/p' /proc/driver/nvidia/version | head -n1)"
                    fi
                    if [ -z "$robo_nix_nvidia_version" ]; then
                      printf '%s\n' "robo-nix: hostGraphics = \"nixgl-nvidia\" could not detect the NVIDIA driver version." >&2
                      printf '%s\n' "robo-nix: set ROBO_NIX_NVIDIA_VERSION to the host driver version, for example 580.65.06." >&2
                      return 1 2>/dev/null || exit 1
                    fi
                    robo_nix_nixgl_store="$(nix-build --no-out-link "${nixglNvidiaSource}" -A auto.nixGLNvidia --arg pkgs '${nixglNvidiaPkgsArg}' --argstr nvidiaVersion "$robo_nix_nvidia_version" --arg enable32bits false)" || {
                      printf '%s\n' "robo-nix: failed to build nixGLNvidia for NVIDIA driver $robo_nix_nvidia_version." >&2
                      return 1 2>/dev/null || exit 1
                    }
                    for robo_nix_nixgl_candidate in "$robo_nix_nixgl_store"/bin/nixGLNvidia*; do
                      if [ -x "$robo_nix_nixgl_candidate" ]; then
                        robo_nix_nixgl="$robo_nix_nixgl_candidate"
                        break
                      fi
                    done
                  fi
                  if [ -z "$robo_nix_nixgl" ] || [ ! -x "$robo_nix_nixgl" ]; then
                    printf '%s\n' "robo-nix: hostGraphics resolved to \"$robo_nix_host_graphics_policy\" but no matching nixGL wrapper is available." >&2
                    printf '%s\n' "robo-nix: set ROBO_NIX_NIXGL to the nixGL wrapper path for uncommon layouts." >&2
                    return 1 2>/dev/null || exit 1
                  fi

                  robo_nix_runtime_ld_library_path="''${LD_LIBRARY_PATH:-}"
                  unset ${graphicsWrapperUnset}

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
                      ${graphicsWrapperCasePattern})
                        export "$robo_nix_nixgl_entry"
                        ;;
                    esac
                  done < <(env \
                    ${graphicsWrapperEnvScrubArgs} \
                    "$robo_nix_nixgl" env -0)

                  unset robo_nix_nixgl robo_nix_nixgl_candidate robo_nix_nixgl_entry
                  unset robo_nix_nvidia_version robo_nix_nvidia_smi robo_nix_nixgl_store
                  unset robo_nix_nixgl_ld_library_path robo_nix_runtime_ld_library_path
                fi

                unset robo_nix_host_graphics_policy
              ''
              + lib.optionalString (hasComponent "linux-headers") ''

                export ROBO_NIX_LINUX_HEADERS="${pkgs.linuxHeaders}/include"
                robo_nix_prepend_path CPATH "$ROBO_NIX_LINUX_HEADERS"
                robo_nix_prepend_path C_INCLUDE_PATH "$ROBO_NIX_LINUX_HEADERS"
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

                robo_nix_prepend_path PATH "$CUDA_PATH/bin"
                robo_nix_prepend_path CPATH "$CUDA_PATH/include"
                robo_nix_prepend_path LIBRARY_PATH "$CUDA_PATH/lib"
                robo_nix_prepend_path LD_LIBRARY_PATH "$CUDA_PATH/lib"
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
                      robo_nix_prepend_path LD_LIBRARY_PATH "$ROBO_NIX_HOST_LIBCUDA_BRIDGE"
                    else
                      robo_nix_prepend_path LD_LIBRARY_PATH "$robo_nix_cuda_driver_dir"
                    fi
                  fi
                  unset robo_nix_cuda_driver_dir
                fi
                unset -f robo_nix_prepend_path
              ''
              + (spec.shellHook or "");
          };
    });
  };
in {
  inherit mkProjectFlake mkProjectFlakeFromManifest;
}
