{common}: {
  x11-gl = {
    pkgs,
    runtimeLibPath,
    runtimeLibs,
    ...
  }: {
    packages = runtimeLibs ++ [pkgs.vulkan-tools];
    shellInit = common.prependPath "LD_LIBRARY_PATH" runtimeLibPath;
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "x11-gl" [];
  };

  qt6 = {pkgs, ...}: {
    packages = [
      pkgs.qt6.qtbase
      pkgs.qt6.qt5compat
    ];
    shellInit =
      common.exportVars {
        ROBO_NIX_QT_PREFIX_PATH = "${pkgs.qt6.qtbase.dev}:${pkgs.qt6.qt5compat.dev}";
      }
      + ''

        export ROBO_NIX_QT_PLUGIN_PATH="$(${pkgs.qt6.qtbase}/bin/qtpaths6 --query QT_INSTALL_PLUGINS 2>/dev/null || true)"
      '';
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "qt6" [];
  };
}
