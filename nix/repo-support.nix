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
    roboBinary = pkgs.rustPlatform.buildRustPackage {
      pname = "robo";
      version = "0.1.0";
      src = repoSource;
      cargoLock.lockFile = ../Cargo.lock;
      preferLocalBuild = true;
      allowSubstitutes = false;
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
        if [ "''${1:-}" = "--check" ]; then
          shift
          find_nix_files | xargs -0 --no-run-if-empty alejandra --check
          find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shfmt -d
          exit 0
        fi

        find_nix_files | xargs -0 --no-run-if-empty alejandra
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shfmt -w
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
        find_nix_files | xargs -0 --no-run-if-empty deadnix --fail
        while IFS= read -r -d "" file; do
          statix check "$file"
        done < <(find_nix_files)
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shellcheck -x
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shfmt -d
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
        target="path:$target_dir"
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
        touch "$out"
      '';

    cpu-safe-repo-profile-contract = pkgs.runCommand "robo-nix-cpu-safe-profile-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.repo-profile}/bin/repo-profile
      touch "$out"
    '';

    gpu-required-cuda-check-contract = pkgs.runCommand "robo-nix-gpu-required-cuda-check-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.cuda-check}/bin/cuda-check
      cat >"$out" <<'EOF'
      GPU-required validation is intentionally separate from default CPU CI.
      Run `nix run .#cuda-check` or the gpu-smoke workflow on a self-hosted NVIDIA runner.
      EOF
    '';
  });
}
