{
  componentCatalog,
  lib,
  nix-ros-overlay,
  nixpkgs,
}: let
  defaultSupportedSystems = [
    "x86_64-linux"
    "aarch64-linux"
  ];
  normalizeEnvSpec = envName: envSpec:
    envSpec
    // {
      components = envSpec.components or [];
      description = envSpec.description or "${envName} robo-nix environment";
      # NOTE: uv owns the interpreter. This value is only metadata for
      # generated files, diagnostics, and UV_PYTHON.
      pythonVersion = envSpec.pythonVersion or "3.11";
      extraPackages = envSpec.extraPackages or [];
      shellInit = envSpec.shellInit or "";
      bootstrap = envSpec.bootstrap or "";
      doctor = envSpec.doctor or "";
      requiredDirectories = envSpec.requiredDirectories or [];
      requiredFiles = envSpec.requiredFiles or [];
      supportedSystems = lib.unique (envSpec.supportedSystems or defaultSupportedSystems);
      workspaceRoot = envSpec.workspaceRoot or ".";
    };

  mkFlakeFromEnvCatalog = {
    defaultEnvName ? null,
    envs,
  }: let
    normalizedEnvCatalog = lib.mapAttrs normalizeEnvSpec envs;
    envNames = builtins.attrNames normalizedEnvCatalog;
    resolvedDefaultEnvName =
      if defaultEnvName != null
      then defaultEnvName
      else if envNames == []
      then null
      else builtins.head envNames;
    allSystems = lib.unique (
      lib.concatMap
      (envSpec: envSpec.supportedSystems)
      (builtins.attrValues normalizedEnvCatalog)
    );
    forEachSystem = f:
      lib.genAttrs allSystems (system:
        f system (import nixpkgs {inherit system;}));

    mkPerSystem = system: pkgs: let
      pkgsRos = import nixpkgs {
        inherit system;
        overlays = [nix-ros-overlay.overlays.default];
      };

      uniquePackages = packages:
        lib.unique (
          packages
          ++ [
            pkgs.bash
            pkgs.coreutils
            pkgs.findutils
            pkgs.gawk
            pkgs.git
            pkgs.gnugrep
            pkgs.gnused
            pkgs.which
          ]
        );

      runtimeLibs = [
        pkgs.stdenv.cc.cc.lib
        pkgs.dbus.lib
        pkgs.fontconfig
        pkgs.freetype
        pkgs.libGL
        pkgs.glib
        pkgs.xorg.libX11
        pkgs.xorg.libXau
        pkgs.xorg.libxcb
        pkgs.xorg.libXdmcp
        pkgs.xorg.libXext
        pkgs.xorg.libXi
        pkgs.xorg.libXinerama
        pkgs.libxkbcommon
        pkgs.xorg.libXrandr
        pkgs.xorg.libXtst
        pkgs.xorg.libXxf86vm
        pkgs.xorg.libXcursor
        pkgs.xorg.xcbutil
        pkgs.xorg.xcbutilcursor
        pkgs.xorg.xcbutilimage
        pkgs.xorg.xcbutilkeysyms
        pkgs.xorg.xcbutilrenderutil
        pkgs.xorg.xcbutilwm
        pkgs.wayland
        pkgs.zlib
      ];
      runtimeLibPath = lib.makeLibraryPath runtimeLibs;

      mkContext = envName: envSpec: {
        inherit componentCatalog envName envSpec lib pkgs pkgsRos runtimeLibPath runtimeLibs system;
      };

      resolveComponent = ctx: componentName: let
        component =
          if builtins.hasAttr componentName componentCatalog
          then componentCatalog.${componentName} ctx
          else throw "Unknown robo-nix component: ${componentName}";
      in
        {
          bootstrap = "";
          check = "";
          doctor = "";
          gpuRequired = false;
          packages = [];
          requiredDirectories = [];
          requiredFiles = [];
          shellInit = "";
          supportedSystems = allSystems;
        }
        // component
        // {name = componentName;};

      envSupportedOnSystem = envSpec: resolvedComponents:
        lib.elem system envSpec.supportedSystems
        && lib.all (component: lib.elem system component.supportedSystems) resolvedComponents;

      normalizeMergedComponent = merged: {
        inherit (merged) bootstrap checks doctor gpuRequired shellInit;
        packages = uniquePackages merged.packages;
        requiredDirectories = lib.unique merged.requiredDirectories;
        requiredFiles = lib.unique merged.requiredFiles;
      };

      renderRequiredDirectoryChecks = requiredDirectories:
        lib.concatMapStringsSep "\n" (path: ''
          if [ ! -d "$WORKSPACE_ROOT/${path}" ]; then
            bootstrap_error "missing required directory: $WORKSPACE_ROOT/${path}"
            doctor_hint "create $WORKSPACE_ROOT/${path} or point ROBO_NIX_WORKSPACE at the correct checkout" >&2
            doctor_hint "run 'robo doctor' for a full setup report" >&2
            exit 1
          fi
        '')
        requiredDirectories;

      renderRequiredFileChecks = requiredFiles:
        lib.concatMapStringsSep "\n" (path: ''
          if [ ! -f "$WORKSPACE_ROOT/${path}" ]; then
            bootstrap_error "missing required file: $WORKSPACE_ROOT/${path}"
            doctor_hint "restore $WORKSPACE_ROOT/${path} or point ROBO_NIX_WORKSPACE at the correct checkout" >&2
            doctor_hint "run 'robo doctor' for a full setup report" >&2
            exit 1
          fi
        '')
        requiredFiles;

      renderDoctorDirectoryChecks = requiredDirectories:
        lib.concatMapStringsSep "\n" (path: ''
          if [ -d "$WORKSPACE_ROOT/${path}" ]; then
            doctor_ok "workspace directory present: ${path}"
          else
            doctor_error "missing workspace directory: ${path}"
            doctor_hint "create $WORKSPACE_ROOT/${path} or set ROBO_NIX_WORKSPACE to the correct checkout"
          fi
        '')
        requiredDirectories;

      renderDoctorFileChecks = requiredFiles:
        lib.concatMapStringsSep "\n" (path: ''
          if [ -f "$WORKSPACE_ROOT/${path}" ]; then
            doctor_ok "workspace file present: ${path}"
          else
            doctor_error "missing workspace file: ${path}"
            doctor_hint "restore $WORKSPACE_ROOT/${path} or set ROBO_NIX_WORKSPACE to the correct checkout"
          fi
        '')
        requiredFiles;

      renderCheckWorkspaceScaffold = merged: ''
        ${lib.concatMapStringsSep "\n" (path: ''mkdir -p "$ROBO_NIX_WORKSPACE/${path}"'') merged.requiredDirectories}
        ${lib.concatMapStringsSep "\n" (path: ''
            mkdir -p "$(dirname "$ROBO_NIX_WORKSPACE/${path}")"
            : > "$ROBO_NIX_WORKSPACE/${path}"
          '')
          merged.requiredFiles}
      '';

      renderPrintConfig = componentNames: merged:
        lib.concatStringsSep "\n" (
          [
            ''printf "env=%s\n" "$ROBO_NIX_ENV_NAME"''
            ''printf "python=%s\n" "$ROBO_NIX_PYTHON_VERSION"''
            ''printf "system=%s\n" "$ROBO_NIX_SYSTEM"''
            ''printf "workspace=%s\n" "$WORKSPACE_ROOT"''
          ]
          ++ builtins.map
          (componentName: ''printf "component=%s\n" "${componentName}"'')
          componentNames
          ++ builtins.map
          (path: ''printf "required_dir=%s\n" "${path}"'')
          merged.requiredDirectories
          ++ builtins.map
          (path: ''printf "required_file=%s\n" "${path}"'')
          merged.requiredFiles
        );

      mergeComponent = acc: component: {
        bootstrap = acc.bootstrap + component.bootstrap;
        checks =
          acc.checks
          + lib.optionalString (component.check != "") ''
            ${component.check}
          '';
        doctor =
          acc.doctor
          + lib.optionalString (component.doctor != "") ''
            ${component.doctor}
          '';
        gpuRequired = acc.gpuRequired || component.gpuRequired;
        packages = acc.packages ++ component.packages;
        requiredDirectories = acc.requiredDirectories ++ component.requiredDirectories;
        requiredFiles = acc.requiredFiles ++ component.requiredFiles;
        shellInit = acc.shellInit + component.shellInit;
      };

      mkEnvVariant = envName: envSpec: let
        ctx = mkContext envName envSpec;
        resolvedComponents = builtins.map (resolveComponent ctx) envSpec.components;
        componentNames = builtins.map (component: component.name) resolvedComponents;
        projectPackages =
          if builtins.isFunction envSpec.extraPackages
          then envSpec.extraPackages pkgs
          else envSpec.extraPackages;
        projectExtension = {
          check = "";
          gpuRequired = false;
          packages = projectPackages;
          inherit (envSpec) bootstrap doctor requiredDirectories requiredFiles shellInit;
        };
      in
        if !envSupportedOnSystem envSpec resolvedComponents
        then null
        else let
          merged = normalizeMergedComponent (
            builtins.foldl' mergeComponent {
              bootstrap = "";
              checks = "";
              doctor = "";
              gpuRequired = false;
              packages = [];
              requiredDirectories = [];
              requiredFiles = [];
              shellInit = "";
            }
            (resolvedComponents ++ [projectExtension])
          );
          shellName = envName;
          defaultWorkspace = envSpec.workspaceRoot;
          requiredDirectoryChecks = renderRequiredDirectoryChecks merged.requiredDirectories;
          requiredFileChecks = renderRequiredFileChecks merged.requiredFiles;
          exportCommon = ''
            export ROBO_NIX_ENV_NAME="${envName}"
            export ROBO_NIX_ENV_DESCRIPTION=${lib.escapeShellArg envSpec.description}
            export ROBO_NIX_DEFAULT_WORKSPACE=${lib.escapeShellArg defaultWorkspace}
            export ROBO_NIX_SYSTEM="${system}"
            export ROBO_NIX_SUPPORTED_SYSTEMS=${lib.escapeShellArg (lib.concatStringsSep " " envSpec.supportedSystems)}
            export ROBO_NIX_COMPONENTS=${lib.escapeShellArg (lib.concatStringsSep " " componentNames)}
            workspace_input="''${ROBO_NIX_WORKSPACE:-${lib.escapeShellArg defaultWorkspace}}"
            WORKSPACE_ROOT="$(realpath -m "$workspace_input")"
            export WORKSPACE_ROOT
            ${merged.shellInit}
          '';
          printConfig = renderPrintConfig componentNames merged;
          doctorDirectoryChecks = renderDoctorDirectoryChecks merged.requiredDirectories;
          doctorFileChecks = renderDoctorFileChecks merged.requiredFiles;
          bootstrapPackage = pkgs.writeShellApplication {
            name = shellName;
            runtimeInputs = merged.packages;
            excludeShellChecks = [
              "SC1091"
              "SC2155"
            ];
            text = ''
              set -euo pipefail
              ${exportCommon}

              mode="bootstrap"
              if [ "''${1:-}" = "--print-config" ]; then
                ${printConfig}
                exit 0
              fi
              if [ "''${1:-}" = "--doctor" ]; then
                mode="doctor"
              fi
              if [ "''${1:-}" = "--dry-run" ]; then
                mode="dry-run"
              fi

              issues=0
              warnings=0

              if { [ "''${ROBO_NIX_COLOR:-}" = "1" ] || { [ -z "''${NO_COLOR:-}" ] && [ -t 1 ]; }; } && [ "''${ROBO_NIX_COLOR:-}" != "0" ]; then
                c_ok="$(printf '\033[32;1m')"
                c_warn="$(printf '\033[33;1m')"
                c_error="$(printf '\033[31;1m')"
                c_hint="$(printf '\033[2m')"
                c_status="$(printf '\033[36;1m')"
                c_reset="$(printf '\033[0m')"
              else
                c_ok=""
                c_warn=""
                c_error=""
                c_hint=""
                c_status=""
                c_reset=""
              fi

              doctor_ok() {
                printf "doctor: %sok:%s %s\n" "$c_ok" "$c_reset" "$1"
              }

              doctor_warn() {
                warnings=$((warnings + 1))
                printf "doctor: %swarn:%s %s\n" "$c_warn" "$c_reset" "$1"
              }

              doctor_error() {
                issues=$((issues + 1))
                printf "doctor: %serror:%s %s\n" "$c_error" "$c_reset" "$1"
              }

              doctor_hint() {
                printf "doctor: %shint:%s %s\n" "$c_hint" "$c_reset" "$1"
              }

              doctor_next() {
                printf "doctor: %snext:%s %s\n" "$c_status" "$c_reset" "$1"
              }

              bootstrap_error() {
                printf "%sbootstrap error:%s %s\n" "$c_error" "$c_reset" "$1" >&2
              }

              run_doctor() {
                printf "doctor: env=%s\n" "$ROBO_NIX_ENV_NAME"
                printf "doctor: python=%s\n" "$ROBO_NIX_PYTHON_VERSION"
                printf "doctor: system=%s\n" "$ROBO_NIX_SYSTEM"
                printf "doctor: workspace=%s\n" "$WORKSPACE_ROOT"

                if [ -d "$WORKSPACE_ROOT" ]; then
                  doctor_ok "workspace root exists"
                else
                  doctor_error "workspace root does not exist"
                  doctor_hint "create $WORKSPACE_ROOT or set ROBO_NIX_WORKSPACE to the correct checkout"
                fi

                ${doctorDirectoryChecks}
                ${doctorFileChecks}
                ${merged.doctor}

                if [ "$issues" -eq 0 ]; then
                  doctor_next "run 'robo dry-run' if you want a bootstrap-only validation pass"
                  doctor_next "run 'robo develop' to enter the environment"
                  printf "doctor: status=ok warnings=%s\n" "$warnings"
                  return 0
                fi

                doctor_next "fix the issues above and rerun 'robo doctor'"
                printf "doctor: status=error issues=%s warnings=%s\n" "$issues" "$warnings" >&2
                return 1
              }

              if [ "$mode" = "doctor" ]; then
                run_doctor
                exit $?
              fi

              if [ ! -d "$WORKSPACE_ROOT" ]; then
                bootstrap_error "workspace not found: $WORKSPACE_ROOT"
                doctor_hint "set ROBO_NIX_WORKSPACE to the project checkout you want to bootstrap" >&2
                doctor_hint "run 'robo doctor' first if you are not sure what is missing" >&2
                exit 1
              fi

              ${requiredDirectoryChecks}
              ${requiredFileChecks}

              if [ "$mode" = "dry-run" ]; then
                printf "validated %s with Python %s on %s at %s\n" \
                  "$ROBO_NIX_ENV_NAME" \
                  "$ROBO_NIX_PYTHON_VERSION" \
                  "$ROBO_NIX_SYSTEM" \
                  "$WORKSPACE_ROOT"
                exit 0
              fi

              mkdir -p "$WORKSPACE_ROOT/.robo-nix"
              ${merged.bootstrap}

              if [ -z "''${ROBO_NIX_QUIET:-}" ]; then
                printf "bootstrapped %s with Python %s on %s at %s\n" \
                  "$ROBO_NIX_ENV_NAME" \
                  "$ROBO_NIX_PYTHON_VERSION" \
                  "$ROBO_NIX_SYSTEM" \
                  "$WORKSPACE_ROOT"
              fi
            '';
          };
          shell = pkgs.mkShell {
            inherit (merged) packages;
            shellHook = ''
              ${exportCommon}
              if [ -d "$WORKSPACE_ROOT" ]; then
                mkdir -p "$WORKSPACE_ROOT/.robo-nix"
              fi
              if [ -z "''${ROBO_NIX_QUIET:-}" ]; then
                echo "robo-nix env: $ROBO_NIX_ENV_NAME"
                echo "python: uv-managed $ROBO_NIX_PYTHON_VERSION"
                echo "system: $ROBO_NIX_SYSTEM"
                echo "workspace: $WORKSPACE_ROOT"
              fi
            '';
          };
          check =
            pkgs.runCommand "check-${shellName}" {
              nativeBuildInputs = [
                bootstrapPackage
                pkgs.bash
                pkgs.coreutils
                pkgs.gnugrep
              ];
            } ''
              export ROBO_NIX_WORKSPACE="$TMPDIR/workspace"
              mkdir -p "$ROBO_NIX_WORKSPACE"
              ${renderCheckWorkspaceScaffold merged}

              report="$TMPDIR/${shellName}.config"
              dryrun="$TMPDIR/${shellName}.dryrun"
              doctor="$TMPDIR/${shellName}.doctor"

              ${bootstrapPackage}/bin/${shellName} --print-config > "$report"
              ${bootstrapPackage}/bin/${shellName} --dry-run > "$dryrun"
              ${lib.optionalString (!merged.gpuRequired) ''
                ${bootstrapPackage}/bin/${shellName} --doctor > "$doctor"
              ''}

              grep -F "env=${envName}" "$report"
              grep -F "python=${envSpec.pythonVersion}" "$report"
              grep -F "system=${system}" "$report"
              grep -F "workspace=$ROBO_NIX_WORKSPACE" "$report"
              grep -F "validated ${envName} with Python ${envSpec.pythonVersion} on ${system}" "$dryrun"
              ${lib.optionalString (!merged.gpuRequired) ''
                grep -F "doctor: status=ok" "$doctor"
              ''}
              ${merged.checks}

              touch "$out"
            '';
        in {
          inherit bootstrapPackage check shell shellName;
          inherit (envSpec) description;
        };

      envVariants = lib.filterAttrs (_: variant: variant != null) (
        lib.mapAttrs mkEnvVariant normalizedEnvCatalog
      );

      appEntries = lib.mapAttrs' (_: variant:
        lib.nameValuePair variant.shellName {
          type = "app";
          program = "${variant.bootstrapPackage}/bin/${variant.shellName}";
          meta.description = variant.description;
        })
      envVariants;

      packageEntries = lib.mapAttrs' (_: variant:
        lib.nameValuePair variant.shellName variant.bootstrapPackage)
      envVariants;

      checkEntries = lib.mapAttrs' (_: variant:
        lib.nameValuePair variant.shellName variant.check)
      envVariants;

      shellEntries = lib.mapAttrs' (_: variant:
        lib.nameValuePair variant.shellName variant.shell)
      envVariants;
    in {
      apps =
        appEntries
        // lib.optionalAttrs (
          resolvedDefaultEnvName
          != null
          && builtins.hasAttr resolvedDefaultEnvName appEntries
        ) {
          default = appEntries.${resolvedDefaultEnvName};
        };
      checks = checkEntries;
      devShells =
        shellEntries
        // lib.optionalAttrs (
          resolvedDefaultEnvName
          != null
          && builtins.hasAttr resolvedDefaultEnvName shellEntries
        ) {
          default = shellEntries.${resolvedDefaultEnvName};
        };
      formatter = pkgs.alejandra;
      packages =
        packageEntries
        // lib.optionalAttrs (
          resolvedDefaultEnvName
          != null
          && builtins.hasAttr resolvedDefaultEnvName packageEntries
        ) {
          default = packageEntries.${resolvedDefaultEnvName};
        };
    };
    perSystem = forEachSystem mkPerSystem;
  in {
    apps = lib.mapAttrs (_: value: value.apps) perSystem;
    checks = lib.mapAttrs (_: value: value.checks) perSystem;
    devShells = lib.mapAttrs (_: value: value.devShells) perSystem;
    formatter = lib.mapAttrs (_: value: value.formatter) perSystem;
    packages = lib.mapAttrs (_: value: value.packages) perSystem;
  };

  mkProjectFlake = spec @ {envName, ...}:
    mkFlakeFromEnvCatalog {
      defaultEnvName = envName;
      envs = {
        "${envName}" = builtins.removeAttrs spec ["envName"];
      };
    };
in {
  inherit mkFlakeFromEnvCatalog mkProjectFlake normalizeEnvSpec;
}
