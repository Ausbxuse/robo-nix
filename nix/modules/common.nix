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
        case ":''${LD_LIBRARY_PATH:-}:" in
          *":$robo_nix_cuda_driver_dir:"*) ;;
          *) export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}$robo_nix_cuda_driver_dir" ;;
        esac
      fi
      unset robo_nix_cuda_driver_dir
    fi
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
