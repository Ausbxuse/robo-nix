{
  common,
  lib,
}: {
  python-uv = {
    envSpec,
    nixpkgsPythonPackages ? {},
    pkgs,
    ...
  }: let
    pythonVersionParts = lib.splitString "." envSpec.pythonVersion;
    pythonMajor = builtins.elemAt pythonVersionParts 0;
    pythonMinor = builtins.elemAt pythonVersionParts 1;
    pythonMajorMinor = "${pythonMajor}.${pythonMinor}";
    pythonAttr = "python${pythonMajor}${pythonMinor}";
    python =
      if builtins.hasAttr envSpec.pythonVersion nixpkgsPythonPackages
      then builtins.getAttr envSpec.pythonVersion nixpkgsPythonPackages
      else if builtins.hasAttr pythonMajorMinor nixpkgsPythonPackages
      then builtins.getAttr pythonMajorMinor nixpkgsPythonPackages
      else if builtins.hasAttr pythonAttr pkgs
      then builtins.getAttr pythonAttr pkgs
      else throw "Unsupported robo-nix pythonVersion ${envSpec.pythonVersion}: nixpkgs-python does not provide ${envSpec.pythonVersion} or ${pythonMajorMinor}, and nixpkgs does not provide ${pythonAttr}";
    pythonInterpreter = toString python.interpreter;
    pythonBin =
      if lib.hasPrefix "/" pythonInterpreter
      then pythonInterpreter
      else "${python}/bin/${pythonInterpreter}";
  in {
    packages = [
      python
      pkgs.uv
    ];
    shellInit =
      common.exportVars {
        ROBO_NIX_PYTHON = pythonBin;
        ROBO_NIX_PYTHON_MAJOR_MINOR = pythonMajorMinor;
        UV_PYTHON = pythonBin;
        UV_CACHE_DIR = "$WORKSPACE_ROOT/.robo-nix/uv-cache";
        UV_PYTHON_DOWNLOADS = "never";
      }
      + "\n"
      + common.exportDefaults {
        UV_PROJECT_ENVIRONMENT = "$WORKSPACE_ROOT/.venv";
        UV_HTTP_TIMEOUT = "300";
      }
      + ''

        unset PYTHONHOME
        unset PYTHONPATH

        robo_nix_filter_python_flags() {
          filtered=""
          pending_isystem=0
          for word in $1; do
            if [ "$pending_isystem" = "1" ]; then
              pending_isystem=0
              case "$word" in
                /nix/store/*python*/include/python*)
                  continue
                  ;;
              esac
              filtered="$filtered''${filtered:+ }-isystem"
              filtered="$filtered''${filtered:+ }$word"
              continue
            fi

            case "$word" in
              -I/nix/store/*python*/include/python* | -L/nix/store/*python*/lib)
                continue
                ;;
              -isystem)
                pending_isystem=1
                continue
                ;;
            esac

            filtered="$filtered''${filtered:+ }$word"
          done
          if [ "$pending_isystem" = "1" ]; then
            filtered="$filtered''${filtered:+ }-isystem"
          fi
          printf '%s' "$filtered"
        }

        export NIX_CFLAGS_COMPILE="$(robo_nix_filter_python_flags "''${NIX_CFLAGS_COMPILE:-}")"
        export NIX_LDFLAGS="$(robo_nix_filter_python_flags "''${NIX_LDFLAGS:-}")"

        venv_dir="''${UV_PROJECT_ENVIRONMENT:-$WORKSPACE_ROOT/.venv}"
        site_packages="$venv_dir/lib/python''${ROBO_NIX_PYTHON_MAJOR_MINOR}/site-packages"
        torch_lib="$site_packages/torch/lib"
        case ":''${LD_LIBRARY_PATH:-}:" in
          *":$torch_lib:"*) ;;
          *) export LD_LIBRARY_PATH="$torch_lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
        esac

        for cmake_prefix in "$site_packages" "$site_packages"/pybind11 "$site_packages"/nanobind; do
          case ":''${CMAKE_PREFIX_PATH:-}:" in
            *":$cmake_prefix:"*) ;;
            *) export CMAKE_PREFIX_PATH="$cmake_prefix''${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}" ;;
          esac
        done

        if [ -d "$site_packages" ]; then
          for cmake_prefix in "$site_packages" "$site_packages"/*; do
            if [ -d "$cmake_prefix/share/cmake" ]; then
              export CMAKE_PREFIX_PATH="$cmake_prefix''${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"
            fi
          done
        fi

        if [ -d "$venv_dir/bin" ]; then
          if [ -n "''${VIRTUAL_ENV:-}" ] && [ "$VIRTUAL_ENV" != "$venv_dir" ]; then
            PATH="''${PATH/#$VIRTUAL_ENV\/bin:/}"
            PATH="''${PATH//:$VIRTUAL_ENV\/bin:/:}"
            PATH="''${PATH/%:$VIRTUAL_ENV\/bin/}"
          fi
          export VIRTUAL_ENV="$venv_dir"
          case ":$PATH:" in
            *":$venv_dir/bin:"*) ;;
            *) export PATH="$venv_dir/bin:$PATH" ;;
          esac
          venv_python="$(readlink -f "$venv_dir/bin/python" 2>/dev/null || true)"
          if [ -n "$venv_python" ] && [ "$venv_python" != "$ROBO_NIX_PYTHON" ]; then
            printf "warn: existing uv environment was not created from the current robo-nix Python: %s\n" "$venv_python" >&2
            printf "hint: expected %s\n" "$ROBO_NIX_PYTHON" >&2
            printf '%s\n' "hint: run 'uv venv --python \"$ROBO_NIX_PYTHON\" --clear' and then 'uv sync'" >&2
          fi
          unset venv_python
        fi
      '';
    check = common.mkComponentCheck "python-uv" [];
  };
}
