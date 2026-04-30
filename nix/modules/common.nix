{lib}: let
  linuxSystems = [
    "x86_64-linux"
    "aarch64-linux"
  ];

  darwinSystems = [
    "x86_64-darwin"
    "aarch64-darwin"
  ];

  allSystems = linuxSystems ++ darwinSystems;
  x86LinuxSystems = ["x86_64-linux"];

  mkComponentCheck = componentName: extraPatterns:
    lib.concatStringsSep "\n" (
      [''grep -F "component=${componentName}" "$report"'']
      ++ builtins.map (pattern: ''grep -F "${pattern}" "$report"'') extraPatterns
    );

  exportVars = vars:
    lib.concatStringsSep "\n" (
      lib.mapAttrsToList (name: value: ''export ${name}="${toString value}"'') vars
    );

  exportDefaults = vars:
    lib.concatStringsSep "\n" (
      lib.mapAttrsToList (name: value: ''export ${name}="''${${name}:-${toString value}}"'') vars
    );

  prependPath = name: value: let
    shellRef = "$" + name;
  in ''export ${name}="${toString value}''${${name}:+:${shellRef}}"'';

  hostCudaDriverShellInit = ''
    robo_nix_find_host_libcuda_dir() {
      if [ -n "''${ROBO_NIX_LIBCUDA_PATH:-}" ]; then
        if [ -f "$ROBO_NIX_LIBCUDA_PATH" ]; then
          dirname "$ROBO_NIX_LIBCUDA_PATH"
          return 0
        fi
        if [ -d "$ROBO_NIX_LIBCUDA_PATH" ] && [ -e "$ROBO_NIX_LIBCUDA_PATH/libcuda.so.1" ]; then
          printf '%s\n' "$ROBO_NIX_LIBCUDA_PATH"
          return 0
        fi
      fi

      for robo_nix_cuda_driver_dir in \
        /run/opengl-driver/lib \
        /usr/lib64/nvidia \
        /usr/lib/x86_64-linux-gnu \
        /usr/lib/x86_64-linux-gnu/nvidia/current \
        /usr/lib/x86_64-linux-gnu/nvidia \
        /usr/lib/wsl/lib
      do
        if [ -e "$robo_nix_cuda_driver_dir/libcuda.so.1" ]; then
          printf '%s\n' "$robo_nix_cuda_driver_dir"
          return 0
        fi
      done

      for robo_nix_ldconfig in /sbin/ldconfig /usr/sbin/ldconfig ldconfig; do
        if command -v "$robo_nix_ldconfig" >/dev/null 2>&1; then
          robo_nix_libcuda_path="$("$robo_nix_ldconfig" -p 2>/dev/null | awk '/libcuda\.so\.1/{print $NF; exit}')"
          if [ -n "$robo_nix_libcuda_path" ] && [ -e "$robo_nix_libcuda_path" ]; then
            dirname "$robo_nix_libcuda_path"
            return 0
          fi
        fi
      done
      return 1
    }

    if robo_nix_cuda_driver_dir="$(robo_nix_find_host_libcuda_dir)"; then
      export ROBO_NIX_LIBCUDA_PATH="$robo_nix_cuda_driver_dir/libcuda.so.1"
      export TRITON_LIBCUDA_PATH="''${TRITON_LIBCUDA_PATH:-$robo_nix_cuda_driver_dir}"
      case ":''${LD_LIBRARY_PATH:-}:" in
        *":$robo_nix_cuda_driver_dir:"*) ;;
        *) export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}$robo_nix_cuda_driver_dir" ;;
      esac
    fi
    unset robo_nix_cuda_driver_dir robo_nix_libcuda_path robo_nix_ldconfig
    unset -f robo_nix_find_host_libcuda_dir 2>/dev/null || true
  '';

  sourceIfPresent = path: ''
    if [ -f "${path}" ]; then
      . "${path}"
    fi
  '';
in {
  inherit
    allSystems
    darwinSystems
    exportDefaults
    exportVars
    hostCudaDriverShellInit
    linuxSystems
    mkComponentCheck
    prependPath
    sourceIfPresent
    x86LinuxSystems
    ;
}
