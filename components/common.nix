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
    linuxSystems
    mkComponentCheck
    prependPath
    sourceIfPresent
    x86LinuxSystems
    ;
}
