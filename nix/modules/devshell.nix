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

  native-build = {pkgs, ...}: let
    libcDev = pkgs.stdenv.cc.libc.dev or pkgs.glibc.dev;
  in {
    packages = [
      pkgs.cmake
      pkgs.gmp
      pkgs.gnumake
      pkgs.ninja
      pkgs.openssl
      pkgs.pkg-config
      pkgs.stdenv.cc
    ];
    shellInit =
      common.exportVars {
        CC = "${pkgs.stdenv.cc.targetPrefix}cc";
        CXX = "${pkgs.stdenv.cc.targetPrefix}c++";
        GMP_ROOT = pkgs.gmp;
        OPENSSL_ROOT_DIR = pkgs.openssl.out;
        OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
        OPENSSL_CRYPTO_LIBRARY = "${pkgs.openssl.out}/lib/libcrypto.so";
        OPENSSL_SSL_LIBRARY = "${pkgs.openssl.out}/lib/libssl.so";
        ROBO_NIX_LIBC_DEV = libcDev;
      }
      + "\n"
      + common.prependPath "PATH" "${pkgs.cmake}/bin"
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "${pkgs.gmp}/lib"
      + "\n"
      + common.prependPath "PATH" "${pkgs.gnumake}/bin"
      + "\n"
      + common.prependPath "PATH" "${pkgs.ninja}/bin"
      + "\n"
      + common.prependPath "PATH" "${pkgs.pkg-config}/bin"
      + "\n"
      + common.prependPath "PATH" "${pkgs.stdenv.cc}/bin";
    check = common.mkComponentCheck "native-build" [];
  };

  media = {pkgs, ...}: let
    ffmpeg = pkgs.ffmpeg_7 or pkgs.ffmpeg;
  in {
    packages = [
      ffmpeg
    ];
    shellInit =
      common.exportVars {
        ROBO_NIX_FFMPEG_ROOT = ffmpeg;
        ROBO_NIX_FFMPEG_LIB = "${ffmpeg.lib}/lib";
      }
      + "\n"
      + common.prependPath "LD_LIBRARY_PATH" "${ffmpeg.lib}/lib";
    check = common.mkComponentCheck "media" [];
  };

  linux-headers = {pkgs, ...}: {
    packages = [
      pkgs.linuxHeaders
    ];
    shellInit =
      common.exportVars {
        ROBO_NIX_LINUX_HEADERS = "${pkgs.linuxHeaders}/include";
      }
      + "\n"
      + common.prependPath "CPATH" "$ROBO_NIX_LINUX_HEADERS";
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
        ROBO_NIX_MUJOCO_GL_DEFAULT = "egl";
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
