{
  description = "Robot learning runtime environment";

  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = ["https://nixpkgs-python.cachix.org"];
    extra-trusted-public-keys = [
      "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-python = {
      url = "github:cachix/nixpkgs-python";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    nixpkgs-python,
    ...
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
  in {
    devShells = forAllSystems (system: let
      pkgs = import nixpkgs {inherit system;};
      lib = pkgs.lib;

      rawPythonVersion = lib.strings.removeSuffix "\n" (builtins.readFile ./.python-version);
      pythonVersionParts = lib.splitString "." rawPythonVersion;
      pythonMajorMinor = lib.concatStringsSep "." (lib.take 2 pythonVersionParts);
      pythonPackages = nixpkgs-python.packages.${system};
      python =
        if builtins.hasAttr rawPythonVersion pythonPackages
        then builtins.getAttr rawPythonVersion pythonPackages
        else if builtins.hasAttr pythonMajorMinor pythonPackages
        then builtins.getAttr pythonMajorMinor pythonPackages
        else throw "robo-nix: nixpkgs-python does not provide Python ${rawPythonVersion} for ${system}";

      spec = import ./robo.nix;
      selectedComponents = spec.components or [];
      extraPackages = spec.extraPackages or (_: []);
      extraRuntimeLibraries = spec.extraRuntimeLibraries or (_: []);

      componentPackages = {
        python-uv = [python pkgs.uv];
        native-build = [pkgs.cmake pkgs.pkg-config pkgs.stdenv.cc];
        desktop-gl = [
          pkgs.glib
          pkgs.libGL
          pkgs.xorg.libICE
          pkgs.xorg.libSM
          pkgs.xorg.libX11
          pkgs.xorg.libXext
          pkgs.xorg.libXrender
        ];
      };

      componentRuntimeLibraries = {
        python-uv = [];
        native-build = [];
        desktop-gl = componentPackages.desktop-gl;
      };

      unknownComponents = lib.filter (component: !(builtins.hasAttr component componentPackages)) selectedComponents;
      componentPackageLists = map (component: builtins.getAttr component componentPackages) selectedComponents;
      componentRuntimeLibraryLists = map (component: builtins.getAttr component componentRuntimeLibraries) selectedComponents;
      runtimeLibraries = (builtins.concatLists componentRuntimeLibraryLists) ++ extraRuntimeLibraries pkgs;
      runtimeLibraryPath = lib.makeLibraryPath runtimeLibraries;
    in {
      default =
        if unknownComponents != []
        then throw "robo-nix: unknown components in robo.nix: ${lib.concatStringsSep ", " unknownComponents}"
        else
          pkgs.mkShell {
            packages = (builtins.concatLists componentPackageLists) ++ extraPackages pkgs;

            shellHook =
              ''
                export ROBO_NIX_PYTHON="${python}/bin/python"
                export UV_PYTHON="$ROBO_NIX_PYTHON"
                export UV_PYTHON_DOWNLOADS=never
                export UV_PROJECT_ENVIRONMENT="''${UV_PROJECT_ENVIRONMENT:-$PWD/.venv}"
                export UV_CACHE_DIR="''${UV_CACHE_DIR:-$PWD/.robo-nix/uv-cache}"
                unset PYTHONHOME
                unset PYTHONPATH

                if [ -d "$UV_PROJECT_ENVIRONMENT/bin" ]; then
                  export VIRTUAL_ENV="$UV_PROJECT_ENVIRONMENT"
                  case ":$PATH:" in
                    *":$UV_PROJECT_ENVIRONMENT/bin:"*) ;;
                    *) export PATH="$UV_PROJECT_ENVIRONMENT/bin:$PATH" ;;
                  esac
                fi
              ''
              + lib.optionalString (runtimeLibraryPath != "") ''

                case ":''${LD_LIBRARY_PATH:-}:" in
                  *":${runtimeLibraryPath}:"*) ;;
                  *) export LD_LIBRARY_PATH="${runtimeLibraryPath}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
                esac
              ''
              + (spec.shellHook or "");
          };
    });
  };
}
