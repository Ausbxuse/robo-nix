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

  mujoco = {pkgs, ...}: {
    packages = [
      pkgs.mujoco
    ];
    shellInit =
      common.exportVars {
        MUJOCO_PATH = pkgs.mujoco;
      }
      + "\n"
      + common.exportDefaults {
        MUJOCO_GL = "egl";
      };
    supportedSystems = common.linuxSystems;
    check = common.mkComponentCheck "mujoco" [];
  };

  isaac-sim = _: {
    # TODO(robo): keep this as a workspace hook until common Isaac packaging
    # behavior is proven across real downstream projects.
    shellInit = common.exportVars {
      ISAAC_SIM_ROOT = "$WORKSPACE_ROOT/third_party/isaac-sim";
      OMNI_KIT_ROOT = "$ISAAC_SIM_ROOT";
    };
    requiredDirectories = ["third_party/isaac-sim"];
    supportedSystems = common.x86LinuxSystems;
    check = common.mkComponentCheck "isaac-sim" ["required_dir=third_party/isaac-sim"];
  };
}
