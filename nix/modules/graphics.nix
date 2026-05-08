{common}: {
  x11-gl = {
    pkgs,
    runtimeLibPath,
    runtimeLibs,
    ...
  }: {
    packages = runtimeLibs ++ [pkgs.vulkan-tools];
    shellInit =
      common.prependPath "LD_LIBRARY_PATH" runtimeLibPath
      + "\n"
      + common.exportDefaults {
        __EGL_VENDOR_LIBRARY_FILENAMES = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
      };
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "x11-gl" [];
  };

  wayland-gl = {
    pkgs,
    runtimeLibPath,
    runtimeLibs,
    ...
  }: {
    packages = runtimeLibs ++ [pkgs.vulkan-tools];
    shellInit =
      common.prependPath "LD_LIBRARY_PATH" runtimeLibPath
      + "\n"
      + common.exportDefaults {
        __EGL_VENDOR_LIBRARY_FILENAMES = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
      };
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "wayland-gl" [];
  };

  matplotlib-qt = _: {
    shellInit = common.exportDefaults {
      MPLBACKEND = "QtAgg";
    };
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "matplotlib-qt" [];
  };

  qt6 = {pkgs, ...}: let
    qtCmakePrefix = "${pkgs.qt6.qtbase.dev}:${pkgs.qt6.qt5compat.dev}";
  in {
    packages = [
      pkgs.qt6.qtbase
      pkgs.qt6.qt5compat
    ];
    shellInit =
      common.prependPath "CMAKE_PREFIX_PATH" qtCmakePrefix
      + ''

        robo_nix_qt_plugin_path="$(${pkgs.qt6.qtbase}/bin/qtpaths6 --query QT_INSTALL_PLUGINS 2>/dev/null || true)"
        if [ -n "$robo_nix_qt_plugin_path" ]; then
          export QT_PLUGIN_PATH="$robo_nix_qt_plugin_path''${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}"
        fi
        unset robo_nix_qt_plugin_path
      '';
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "qt6" [];
  };
}
