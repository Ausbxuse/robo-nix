{common}: let
  hostNvidiaRuntime = ''
    robo_nix_prepend_existing_path() {
      var_name="$1"
      shift
      current_value="$(printenv "$var_name" || true)"
      new_value=""
      for entry in "$@"; do
        [ -d "$entry" ] || continue
        case ":$new_value:$current_value:" in
          *":$entry:"*) ;;
          *)
            if [ -n "$new_value" ]; then
              new_value="$new_value:$entry"
            else
              new_value="$entry"
            fi
            ;;
        esac
      done
      if [ -n "$new_value" ]; then
        export "$var_name=$new_value''${current_value:+:$current_value}"
      fi
    }

    if [ -z "''${VK_ICD_FILENAMES:-}" ]; then
      for robo_nix_vk_icd in \
        /run/opengl-driver/share/vulkan/icd.d/nvidia_icd.x86_64.json \
        /usr/share/vulkan/icd.d/nvidia_icd.json \
        /usr/share/vulkan/icd.d/nvidia_icd.x86_64.json \
        /etc/vulkan/icd.d/nvidia_icd.json
      do
        if [ -f "$robo_nix_vk_icd" ]; then
          export VK_ICD_FILENAMES="$robo_nix_vk_icd"
          break
        fi
      done
    fi

    if [ -z "''${__EGL_VENDOR_LIBRARY_FILENAMES:-}" ]; then
      for robo_nix_egl_vendor in \
        /run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json \
        /usr/share/glvnd/egl_vendor.d/10_nvidia.json \
        /usr/share/glvnd/egl_vendor.d/50_nvidia.json
      do
        if [ -f "$robo_nix_egl_vendor" ]; then
          export __EGL_VENDOR_LIBRARY_FILENAMES="$robo_nix_egl_vendor"
          break
        fi
      done
    fi

    robo_nix_prepend_existing_path LD_LIBRARY_PATH \
      /run/opengl-driver/lib \
      /run/opengl-driver-32/lib \
      /usr/lib/x86_64-linux-gnu/nvidia/current \
      /usr/lib/x86_64-linux-gnu/nvidia \
      /usr/lib/nvidia \
      /usr/lib/nvidia-590 \
      /usr/lib/nvidia-580 \
      /usr/lib/nvidia-575 \
      /usr/lib/nvidia-570 \
      /usr/lib/wsl/lib

    robo_nix_prepend_existing_path XDG_DATA_DIRS \
      /run/opengl-driver/share \
      /usr/share

    unset -f robo_nix_prepend_existing_path
    unset robo_nix_vk_icd robo_nix_egl_vendor
  '';
in {
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
      + hostNvidiaRuntime;
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
