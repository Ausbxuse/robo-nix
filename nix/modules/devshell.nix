{common}: {
  base = {
    envSpec,
    pkgs,
    system,
    ...
  }: let
    certFile = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
  in {
    packages = [
      pkgs.cacert
      pkgs.git
      pkgs.just
      pkgs.ripgrep
      pkgs.which
    ];
    shellInit = common.exportVars {
      ROBO_NIX_SYSTEM = system;
      ROBO_NIX_PYTHON_VERSION = envSpec.pythonVersion;
      SSL_CERT_FILE = certFile;
      NIX_SSL_CERT_FILE = certFile;
      REQUESTS_CA_BUNDLE = certFile;
      CURL_CA_BUNDLE = certFile;
      GIT_SSL_CAINFO = certFile;
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
    shellInit = common.exportDefaults {
      OMNI_KIT_ACCEPT_EULA = "Y";
    };
    supportedSystems = common.x86LinuxSystems;
    check = common.mkComponentCheck "isaac-sim" [];
  };
}
