{
  allSystems,
  componentMetadata,
  lib,
  nixpkgs,
  profileMetadata,
  repoRoot,
  runtimeInference,
}: let
  repoPath = toString repoRoot;
  sourceFilter = path: _type: let
    rel = lib.removePrefix (repoPath + "/") (toString path);
    top = builtins.head (lib.splitString "/" rel);
  in
    !(builtins.elem top [
      ".git"
      ".github"
      "playgrounds"
      "target"
    ]);
  repoSource = lib.sources.cleanSourceWith {
    src = repoRoot;
    filter = sourceFilter;
  };
  forEachSystem = f:
    lib.genAttrs allSystems (system:
      f system (import nixpkgs {inherit system;}));
in rec {
  repoPackages = forEachSystem (_: pkgs: let
    defaultSourceUrl = "github:ausbxuse/robo-nix";
    repoTargetPrelude = ''
      target_dir="''${ROBO_NIX_REPO_ROOT:-$PWD}"
      if [ ! -f "$target_dir/flake.nix" ]; then
        target_dir=${lib.escapeShellArg repoPath}
      fi
      cd "$target_dir"
    '';
    componentManifest = pkgs.writeText "robo-nix-component-manifest.json" (builtins.toJSON {
      components = componentMetadata;
      profiles = profileMetadata;
      inherit runtimeInference;
    });
    buildNpmPackage = pkgs.buildNpmPackage.override {
      nodejs = pkgs.nodejs_20;
    };
    roboBinary = pkgs.rustPlatform.buildRustPackage {
      pname = "robo";
      version = "0.1.0";
      src = repoSource;
      cargoLock.lockFile = ../Cargo.lock;
      preferLocalBuild = true;
      allowSubstitutes = false;
    };
    vitepressTool = buildNpmPackage {
      pname = "robo-nix-docs-tool";
      version = "0.1.0";
      src = repoSource + "/docs";
      npmDepsHash = "sha256-T/jvtsEg7fHkGhoJHNhj0DAawFYhjMfBzcKFcjc0s2o=";
      dontNpmBuild = true;
      installPhase = ''
        runHook preInstall
        mkdir -p "$out"
        cp -R node_modules "$out/"
        runHook postInstall
      '';
    };
    roboCli =
      (pkgs.writeShellApplication {
        name = "robo";
        runtimeInputs = [
          pkgs.git
          pkgs.nix
          pkgs.uv
        ];
        text = ''
          export ROBO_NIX_COMPONENT_MANIFEST="${componentManifest}"
          export ROBO_NIX_DEFAULT_SOURCE_URL="${defaultSourceUrl}"
          export ROBO_NIX_RUNTIME_SOURCE_URL="path:${repoSource}"
          exec ${roboBinary}/bin/robo "$@"
        '';
      })
      .overrideAttrs (_: {
        preferLocalBuild = true;
        allowSubstitutes = false;
      });
  in {
    repo-fmt = pkgs.writeShellApplication {
      name = "repo-fmt";
      runtimeInputs = [
        pkgs.alejandra
        pkgs.findutils
        pkgs.shfmt
      ];
      text = ''
        set -euo pipefail
        ${repoTargetPrelude}
        find_nix_files() {
          find . \
            -path ./playgrounds -prune -o \
            -path ./target -prune -o \
            -type f -name '*.nix' -print0
        }
        find_shell_files() {
          find tests -type f -name '*.sh' -print0
          if [ -d scripts ]; then
            find scripts -type f -name '*.sh' -print0
          fi
        }
        if [ "''${1:-}" = "--check" ]; then
          shift
          find_nix_files | xargs -0 --no-run-if-empty alejandra --check
          find_shell_files | xargs -0 --no-run-if-empty shfmt -d
          exit 0
        fi

        find_nix_files | xargs -0 --no-run-if-empty alejandra
        find_shell_files | xargs -0 --no-run-if-empty shfmt -w
      '';
    };

    docs = pkgs.stdenvNoCC.mkDerivation {
      pname = "robo-nix-docs";
      version = "0.1.0";
      src = repoSource;
      nativeBuildInputs = [
        pkgs.nodejs_20
        vitepressTool
      ];
      buildPhase = ''
        runHook preBuild
        cd docs
        CI=1 ${vitepressTool}/node_modules/.bin/vitepress build .
        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p "$out"
        cp -R .vitepress/dist/. "$out/"
        runHook postInstall
      '';
    };

    docs-serve = pkgs.writeShellApplication {
      name = "docs-serve";
      runtimeInputs = [
        pkgs.nodejs_20
        vitepressTool
      ];
      text = ''
        set -euo pipefail
        ${repoTargetPrelude}
        cd docs

        created_node_modules=0
        if [ ! -e node_modules ]; then
          ln -s ${vitepressTool}/node_modules node_modules
          created_node_modules=1
        fi
        vitepress_pid=
        cleanup() {
          if [ -n "$vitepress_pid" ]; then
            kill "$vitepress_pid" 2>/dev/null || true
            wait "$vitepress_pid" 2>/dev/null || true
          fi
          if [ "$created_node_modules" = 1 ]; then
            rm -f node_modules
          fi
        }
        trap cleanup EXIT INT TERM

        ${vitepressTool}/node_modules/.bin/vitepress dev . --host 127.0.0.1 "$@" &
        vitepress_pid=$!
        wait "$vitepress_pid"
      '';
    };

    repo-lint = pkgs.writeShellApplication {
      name = "repo-lint";
      runtimeInputs = [
        pkgs.deadnix
        pkgs.findutils
        pkgs.shellcheck
        pkgs.shfmt
        pkgs.statix
      ];
      text = ''
        set -euo pipefail
        ${repoTargetPrelude}
        find_nix_files() {
          find . \
            -path ./playgrounds -prune -o \
            -path ./target -prune -o \
            -type f -name '*.nix' -print0
        }
        find_shell_files() {
          find tests -type f -name '*.sh' -print0
          if [ -d scripts ]; then
            find scripts -type f -name '*.sh' -print0
          fi
        }
        find_nix_files | xargs -0 --no-run-if-empty deadnix --fail
        while IFS= read -r -d "" file; do
          statix check "$file"
        done < <(find_nix_files)
        find_shell_files | xargs -0 --no-run-if-empty shellcheck -x
        find_shell_files | xargs -0 --no-run-if-empty shfmt -d
      '';
    };

    repo-profile = pkgs.writeShellApplication {
      name = "repo-profile";
      runtimeInputs = [
        pkgs.git
        pkgs.nix
      ];
      excludeShellChecks = ["SC2034"];
      text = ''
        set -euo pipefail
        exec 2>&1

        ${repoTargetPrelude}
        target="git+file://$target_dir"
        TIMEFMT='%J %E %MKB'

        echo "profiling robo-nix at $target"
        echo "nix eval --raw $target#apps.x86_64-linux.default.program"
        time nix eval --raw "$target#apps.x86_64-linux.default.program" >/dev/null
        echo "nix eval --raw $target#packages.x86_64-linux.default.name"
        time nix eval --raw "$target#packages.x86_64-linux.default.name" >/dev/null
        echo "nix flake show $target --all-systems"
        time nix flake show "$target" --all-systems >/dev/null
      '';
    };

    cuda-check = pkgs.writeShellApplication {
      name = "cuda-check";
      runtimeInputs = [
        pkgs.git
        pkgs.nix
        pkgs.uv
      ];
      text = ''
        export ROBO_NIX_COMPONENT_MANIFEST="${componentManifest}"
        export ROBO_NIX_DEFAULT_SOURCE_URL="${defaultSourceUrl}"
        export ROBO_NIX_RUNTIME_SOURCE_URL="path:${repoSource}"
        exec ${roboBinary}/bin/robo cuda-check "$@"
      '';
    };

    robo = pkgs.symlinkJoin {
      name = "robo";
      paths = [roboCli];
      preferLocalBuild = true;
      allowSubstitutes = false;
      postBuild = ''
        mkdir -p \
          "$out/share/bash-completion/completions" \
          "$out/share/zsh/site-functions" \
          "$out/share/fish/vendor_completions.d"

        ${roboCli}/bin/robo completion bash > "$out/share/bash-completion/completions/robo"
        ${roboCli}/bin/robo completion zsh > "$out/share/zsh/site-functions/_robo"
        ${roboCli}/bin/robo completion fish > "$out/share/fish/vendor_completions.d/robo.fish"
      '';
    };
  });

  repoChecks = forEachSystem (_: pkgs: {
    lint-nix =
      pkgs.runCommand "robo-nix-lint-nix" {
        nativeBuildInputs = [
          pkgs.deadnix
          pkgs.statix
        ];
      } ''
        cd ${repoSource}
        deadnix --fail .
        statix check .
        touch "$out"
      '';

    lint-shell =
      pkgs.runCommand "robo-nix-lint-shell" {
        nativeBuildInputs = [
          pkgs.findutils
          pkgs.shellcheck
        ];
      } ''
        cd ${repoSource}
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shellcheck -x
        if [ -d scripts ]; then
          find scripts -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shellcheck -x
        fi
        touch "$out"
      '';

    cpu-safe-repo-profile-contract = pkgs.runCommand "robo-nix-cpu-safe-profile-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.repo-profile}/bin/repo-profile
      touch "$out"
    '';

    docs-build = repoPackages.${pkgs.system}.docs;

    gpu-required-cuda-check-contract = pkgs.runCommand "robo-nix-gpu-required-cuda-check-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.cuda-check}/bin/cuda-check
      cat >"$out" <<'EOF'
      GPU-required validation is intentionally separate from default CPU CI.
      Run `nix run .#cuda-check` or the gpu-smoke workflow on a self-hosted NVIDIA runner.
      EOF
    '';
  });
}
