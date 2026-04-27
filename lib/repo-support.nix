{
  allSystems,
  componentMetadata,
  lib,
  nixpkgs,
  profileMetadata,
  repoRoot,
  runtimeInference,
  vendorMetadata,
}: let
  forEachSystem = f:
    lib.genAttrs allSystems (system:
      f system (import nixpkgs {inherit system;}));
in rec {
  repoPackages = forEachSystem (_: pkgs: let
    repoPath = toString repoRoot;
    repoSource = pkgs.lib.sources.cleanSource repoRoot;
    defaultSourceUrl = "path:${repoSource}";
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
      inherit vendorMetadata;
    });
    roboBinary = pkgs.rustPlatform.buildRustPackage {
      pname = "robo";
      version = "0.1.0";
      src = repoSource;
      cargoLock.lockFile = ../Cargo.lock;
    };
    roboCli = pkgs.writeShellApplication {
      name = "robo";
      runtimeInputs = [
        pkgs.git
        pkgs.nix
        pkgs.uv
      ];
      text = ''
        export ROBO_NIX_COMPONENT_MANIFEST="${componentManifest}"
        export ROBO_NIX_DEFAULT_SOURCE_URL="${defaultSourceUrl}"
        exec ${roboBinary}/bin/robo "$@"
      '';
    };
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
        if [ "''${1:-}" = "--check" ]; then
          shift
          alejandra --check .
          find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shfmt -d
          exit 0
        fi

        alejandra .
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
        deadnix --fail .
        statix check .
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shellcheck -x
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shfmt -d
      '';
    };

    repo-profile = pkgs.writeShellApplication {
      name = "repo-profile";
      runtimeInputs = [pkgs.nix];
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

    cuda-doctor = pkgs.writeShellApplication {
      name = "cuda-doctor";
      runtimeInputs = [
        pkgs.git
        pkgs.nix
        pkgs.uv
      ];
      text = ''
        export ROBO_NIX_COMPONENT_MANIFEST="${componentManifest}"
        export ROBO_NIX_DEFAULT_SOURCE_URL="${defaultSourceUrl}"
        exec ${roboBinary}/bin/robo cuda-doctor "$@"
      '';
    };

    robo = pkgs.symlinkJoin {
      name = "robo";
      paths = [roboCli];
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

  repoChecks = forEachSystem (_: pkgs: let
    lintSrc = pkgs.lib.sources.cleanSource repoRoot;
  in {
    lint-nix =
      pkgs.runCommand "robo-nix-lint-nix" {
        nativeBuildInputs = [
          pkgs.deadnix
          pkgs.statix
        ];
      } ''
        cd ${lintSrc}
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
        cd ${lintSrc}
        find tests -type f -name '*.sh' -print0 | xargs -0 --no-run-if-empty shellcheck -x
        touch "$out"
      '';

    cpu-safe-repo-profile-contract = pkgs.runCommand "robo-nix-cpu-safe-profile-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.repo-profile}/bin/repo-profile
      touch "$out"
    '';

    gpu-required-cuda-doctor-contract = pkgs.runCommand "robo-nix-gpu-required-cuda-doctor-contract" {} ''
      test -x ${repoPackages.${pkgs.system}.cuda-doctor}/bin/cuda-doctor
      cat >"$out" <<'EOF'
      GPU-required validation is intentionally separate from default CPU CI.
      Run `nix run .#cuda-doctor` or the gpu-smoke workflow on a self-hosted NVIDIA runner.
      EOF
    '';
  });
}
