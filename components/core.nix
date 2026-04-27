{common}: {
  base = {
    envSpec,
    pkgs,
    system,
    ...
  }: {
    packages = [
      pkgs.git
      pkgs.just
      pkgs.ripgrep
      pkgs.which
    ];
    shellInit = common.exportVars {
      ROBO_NIX_SYSTEM = system;
      ROBO_NIX_PYTHON_VERSION = envSpec.pythonVersion;
    };
  };

  python-uv = {
    envSpec,
    pkgs,
    ...
  }: {
    packages = [
      pkgs.uv
    ];
    shellInit =
      common.exportVars {
        UV_PYTHON = envSpec.pythonVersion;
        UV_CACHE_DIR = "$WORKSPACE_ROOT/.robo-nix/uv-cache";
      }
      + ''

        if [ -d "$WORKSPACE_ROOT/.venv/bin" ]; then
          export PATH="$WORKSPACE_ROOT/.venv/bin:$PATH"
        fi
      '';
    check = common.mkComponentCheck "python-uv" [];
  };

  native-build = {pkgs, ...}: {
    packages = [
      pkgs.cmake
      pkgs.gnumake
      pkgs.ninja
      pkgs.openssl
      pkgs.pkg-config
      pkgs.stdenv.cc
    ];
    shellInit = common.exportVars {
      CC = "${pkgs.stdenv.cc.targetPrefix}cc";
      CXX = "${pkgs.stdenv.cc.targetPrefix}c++";
      OPENSSL_ROOT_DIR = pkgs.openssl.out;
      OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
      OPENSSL_CRYPTO_LIBRARY = "${pkgs.openssl.out}/lib/libcrypto.so";
      OPENSSL_SSL_LIBRARY = "${pkgs.openssl.out}/lib/libssl.so";
    };
    check = common.mkComponentCheck "native-build" [];
  };

  media = {pkgs, ...}: {
    packages = [
      pkgs.ffmpeg
    ];
    shellInit = common.exportVars {
      ROBO_NIX_FFMPEG_ROOT = pkgs.ffmpeg;
    };
    check = common.mkComponentCheck "media" [];
  };

  linux-headers = {pkgs, ...}: {
    packages = [
      pkgs.linuxHeaders
    ];
    shellInit = common.prependPath "CPATH" "${pkgs.linuxHeaders}/include";
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "linux-headers" [];
  };

  x11-gl = {
    runtimeLibPath,
    runtimeLibs,
    ...
  }: {
    packages = runtimeLibs;
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
