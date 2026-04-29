#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

fast_mode=0
if [ "${1:-}" = "--fast" ]; then
	fast_mode=1
	shift
fi

run_fast_mode() {
	[ "$fast_mode" -eq 1 ]
}

assert_expr() {
	local expr="$1"
	nix eval --impure --expr "$expr" >/dev/null
}

assert_component_catalog_contract() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      components = builtins.attrNames flake.lib.components;
    in
      assert builtins.elem "base" components;
      assert builtins.elem "media" components;
      assert builtins.elem "ros2-jazzy" components;
      assert builtins.elem "isaac-sim" components;
      true
  '
	assert_expr "$expr"
}

assert_component_metadata_contract() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      metadata = flake.lib.componentMetadata;
    in
      assert metadata.base.category == "core";
      assert metadata.isaac-sim.scaffoldDirectories == [ "third_party/isaac-sim" ];
      true
  '
	assert_expr "$expr"
}

assert_profile_metadata_contract() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      profiles = flake.lib.profileMetadata;
    in
      assert profiles.minimal.components == [ "base" "python-uv" "native-build" ];
      assert profiles.ros2-workspace.supportedSystems == [ "x86_64-linux" "aarch64-linux" ];
      assert profiles.isaac-ros2.components == [ "base" "python-uv" "native-build" "x11-gl" "cuda-toolkit" "isaac-sim" "ros2-jazzy" "ros-workspace" ];
      true
  '
	assert_expr "$expr"
}

assert_runtime_inference_contract() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      inference = flake.lib.runtimeInference;
      ruleComponents = builtins.concatMap (rule: rule.components) inference.dependencyRules;
      workspaceRuleComponents = builtins.concatMap (rule: rule.components) inference.workspaceDirectoryRules;
      scriptRuleComponents = builtins.concatMap (rule: rule.components) inference.scriptRules;
      inferredComponents = ruleComponents ++ workspaceRuleComponents ++ scriptRuleComponents;
      knownComponent = name: builtins.hasAttr name flake.lib.componentMetadata;
    in
      assert inference.defaultProfile == "minimal";
      assert builtins.length inference.dependencyRules > 0;
      assert builtins.all knownComponent inferredComponents;
      assert builtins.any (rule: builtins.elem "opencv-python" rule.dependencies && builtins.elem "media" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "pyqt6" rule.dependencies && builtins.elem "qt6" rule.components && builtins.elem "x11-gl" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "cuda-python" rule.dependencies && builtins.elem "cuda-toolkit" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "cupy-cuda12x" rule.dependencies && builtins.elem "cuda-toolkit" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "isaacsim" rule.dependencies && builtins.elem "isaac-sim" rule.components && builtins.elem "cuda-toolkit" rule.components && builtins.elem "x11-gl" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "flash-attn" rule.dependencies && builtins.elem "cuda-toolkit" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "flash-attn" rule.dependencies && builtins.elem "native-build" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "pytorch3d" rule.dependencies && builtins.elem "cuda-toolkit" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "torch3d" rule.dependencies && builtins.elem "cuda-toolkit" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "evdev" rule.dependencies && builtins.elem "linux-headers" rule.components) inference.dependencyRules;
      assert builtins.any (rule: builtins.elem "xrobot" rule.nameContains && builtins.elem "qt6" rule.components) inference.workspaceDirectoryRules;
      assert builtins.elem "bootstrap_" inference.scriptDiscovery.prefixes;
      true
  '
	assert_expr "$expr"
}

assert_cuda_toolkit_uses_requested_wheel_version() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      pkgs = import flake.inputs.nixpkgs {
        system = "x86_64-linux";
        config.allowUnfreePredicate = pkg: true;
      };
      ctx = {
        componentCatalog = flake.lib.components;
        envName = "cuda";
        envSpec = {
          cudaWheelVersion = "12.6";
        };
        lib = flake.inputs.nixpkgs.lib;
        inherit pkgs;
        pkgsRos = pkgs;
        runtimeLibPath = "";
        runtimeLibs = [];
        system = "x86_64-linux";
      };
      component = flake.lib.components."cuda-toolkit" ctx;
      expectedCompiler = pkgs.cudaPackages_12_6.backendStdenv.cc.name;
      packageNames = builtins.map (package: package.name) component.packages;
    in
      assert builtins.elem expectedCompiler packageNames;
      assert builtins.elem "robo-cuda-toolkit-12.6" packageNames;
      true
  '
	assert_expr "$expr"
}

assert_manifest_helpers_contract() {
	local tmpdir
	tmpdir="$(mktemp_dir)"
	trap 'cleanup_dir "$tmpdir"' RETURN

	cat >"$tmpdir/robo.nix" <<EOF
{
  envName = "manifest";
  description = "Manifest fixture";
  components = [
    "base"
    "python-uv"
  ];
  pythonVersion = "3.11";
  supportedSystems = [ "x86_64-linux" ];
  workspaceRoot = ".";
}
EOF

	cat >"$tmpdir/flake.nix" <<EOF
{
  inputs.robo-nix.url = "path:${repo_root}";

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}
EOF

	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${tmpdir}"'";
      manifest = flake.inputs.robo-nix.lib.loadProjectManifest "'"${tmpdir}/robo.nix"'";
    in
      assert manifest.envName == "manifest";
      assert builtins.hasAttr "manifest" flake.apps.x86_64-linux;
      true
  '
	assert_expr "$expr"

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_normalize_defaults() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      normalized = flake.lib.normalizeEnvSpec "demo" {};
    in
      assert normalized.description == "demo robo-nix environment";
      assert normalized.pythonVersion == "3.11";
      assert normalized.workspaceRoot == ".";
      assert normalized.components == [];
      assert normalized.supportedSystems == ["x86_64-linux" "aarch64-linux"];
      true
	'
	assert_expr "$expr"
}

assert_normalize_dedupes() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
      normalized = flake.lib.normalizeEnvSpec "demo" {
        pythonVersion = "3.12";
        supportedSystems = [ "x86_64-linux" "x86_64-linux" "aarch64-linux" ];
      };
    in
      assert normalized.pythonVersion == "3.12";
      assert normalized.supportedSystems == [ "x86_64-linux" "aarch64-linux" ];
      true
  '
	assert_expr "$expr"
}

assert_repo_tooling_outputs() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
    in
      assert builtins.hasAttr "repo-fmt" flake.apps.x86_64-linux;
      assert builtins.hasAttr "repo-lint" flake.apps.x86_64-linux;
      assert builtins.hasAttr "repo-profile" flake.apps.x86_64-linux;
      assert builtins.hasAttr "cuda-check" flake.apps.x86_64-linux;
      assert builtins.hasAttr "robo" flake.apps.x86_64-linux;
      assert builtins.hasAttr "repo-fmt" flake.packages.x86_64-linux;
      assert builtins.hasAttr "repo-lint" flake.packages.x86_64-linux;
      assert builtins.hasAttr "repo-profile" flake.packages.x86_64-linux;
      assert builtins.hasAttr "cuda-check" flake.packages.x86_64-linux;
      assert builtins.hasAttr "robo" flake.packages.x86_64-linux;
      true
  '
	assert_expr "$expr"
}

assert_validation_tier_checks() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
    in
      assert builtins.hasAttr "cpu-safe-repo-profile-contract" flake.checks.x86_64-linux;
      assert builtins.hasAttr "gpu-required-cuda-check-contract" flake.checks.x86_64-linux;
      true
  '
	assert_expr "$expr"
}

assert_component_gating() {
	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${repo_root}"'";
    in
      assert builtins.hasAttr "isaac-ros2-learning" flake.apps.x86_64-linux;
      assert !(builtins.hasAttr "isaac-ros2-learning" flake.apps.aarch64-darwin);
      assert builtins.hasAttr "robot-learning" flake.apps.aarch64-darwin;
      true
  '
	assert_expr "$expr"
}

assert_unknown_component_is_rejected() {
	local tmpdir
	tmpdir="$(mktemp_dir)"
	trap 'cleanup_dir "$tmpdir"' RETURN

	cat >"$tmpdir/flake.nix" <<EOF
{
  inputs = {
    robo-nix.url = "path:${repo_root}";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "broken";
      components = [
        "base"
        "missing-component"
      ];
      supportedSystems = [
        "x86_64-linux"
      ];
    };
}
EOF

	assert_command_fails nix eval "$tmpdir#packages.x86_64-linux.default"

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_project_flake_contract() {
	local tmpdir
	local system
	tmpdir="$(mktemp_dir)"
	system="$(current_nix_system)"
	trap 'cleanup_dir "$tmpdir"' RETURN

	cat >"$tmpdir/flake.nix" <<EOF
{
  inputs = {
    robo-nix.url = "path:${repo_root}";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "contract";
      components = [
        "base"
        "python-uv"
      ];
      description = "Contract fixture";
      pythonVersion = "3.11";
      supportedSystems = [
        "${system}"
      ];
      workspaceRoot = ".";
    };
}
EOF

	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${tmpdir}"'";
      system = "'"${system}"'";
    in
      assert builtins.hasAttr system flake.apps;
      assert builtins.hasAttr "default" flake.apps.${system};
      assert builtins.hasAttr "contract" flake.apps.${system};
      true
  '
	assert_expr "$expr"

	local config_file
	local check_file
	config_file="$tmpdir/contract.config"
	check_file="$tmpdir/contract.check"
	nix run "$tmpdir#default" -- --print-config >"$config_file"
	nix run "$tmpdir#default" -- --check >"$check_file"
	grep -F "env=contract" "$config_file" >/dev/null
	grep -F "python=3.11" "$config_file" >/dev/null
	grep -F "component=base" "$config_file" >/dev/null
	grep -F "component=python-uv" "$config_file" >/dev/null
	grep -F "env=contract" "$check_file" >/dev/null
	grep -F "next: run 'robo activate' to enter the environment" "$check_file" >/dev/null
	grep -F "status=ok" "$check_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_project_extension_contract() {
	local tmpdir
	local system
	tmpdir="$(mktemp_dir)"
	system="$(current_nix_system)"
	trap 'cleanup_dir "$tmpdir"' RETURN

	cat >"$tmpdir/flake.nix" <<EOF
{
  inputs = {
    robo-nix.url = "path:${repo_root}";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "extended";
      components = [
        "base"
      ];
      extraPackages = pkgs: [
        pkgs.ffmpeg
      ];
      shellInit = ''
        export PROJECT_EXTENSION_READY=1
      '';
        bootstrap = ''
          printf "extension bootstrap=%s\n" "\$PROJECT_EXTENSION_READY"
        '';
      diagnostics = ''
        check_ok "project extension check ran"
      '';
      requiredDirectories = [
        "third_party/example"
      ];
      supportedSystems = [
        "${system}"
      ];
    };
}
EOF

	local check_file
	local dryrun_file
	check_file="$tmpdir/extended.check"
	dryrun_file="$tmpdir/extended.dryrun"

	mkdir -p "$tmpdir/third_party/example"
	(
		cd "$tmpdir"
		nix run .#default -- --check >"$check_file"
		nix run .#default -- --dry-run >"$dryrun_file"
	)
	grep -F "ok: project extension check ran" "$check_file" >/dev/null
	grep -F "validated extended with Python 3.11" "$dryrun_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_human_facing_failure_contract() {
	local tmpdir
	local system
	tmpdir="$(mktemp_dir)"
	system="$(current_nix_system)"
	trap 'cleanup_dir "$tmpdir"' RETURN

	cat >"$tmpdir/flake.nix" <<EOF
{
  inputs = {
    robo-nix.url = "path:${repo_root}";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "failing-contract";
      components = [
        "base"
        "python-uv"
        "native-build"
        "ros-workspace"
      ];
      pythonVersion = "3.11";
      supportedSystems = [
        "${system}"
      ];
      workspaceRoot = ".";
    };
}
EOF

	local check_file
	local dryrun_file
	check_file="$tmpdir/failure.check"
	dryrun_file="$tmpdir/failure.dryrun"

	assert_command_fails_capture "$check_file" nix run "$tmpdir#default" -- --check
	grep -F "error: missing workspace directory: ros_ws/src" "$check_file" >/dev/null
	grep -F "hint: create " "$check_file" >/dev/null
	grep -F "next: fix the issues above and rerun 'robo check'" "$check_file" >/dev/null
	grep -F "status=error" "$check_file" >/dev/null

	assert_command_fails_capture "$dryrun_file" nix run "$tmpdir#default" -- --dry-run
	grep -F "bootstrap error: missing required directory:" "$dryrun_file" >/dev/null
	grep -F "hint: run 'robo check' for a full setup report" "$dryrun_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_project_init_validation_contract() {
	local tmpdir
	local output_file
	tmpdir="$(mktemp_dir)"
	output_file="$tmpdir/robo-init.txt"
	trap 'cleanup_dir "$tmpdir"' RETURN

	nix run "path:${repo_root}#robo" -- init --stdout \
		--name invalid-project \
		--components base,python-uv,native-build \
		--python-version 3.10 \
		--systems x86_64-linux >"$output_file"
	grep -F "mkProjectFlakeFromManifest" "$output_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_component_catalog_contract
assert_component_metadata_contract
assert_profile_metadata_contract
assert_runtime_inference_contract
assert_cuda_toolkit_uses_requested_wheel_version
assert_manifest_helpers_contract
assert_normalize_defaults
assert_normalize_dedupes
assert_repo_tooling_outputs
assert_validation_tier_checks
assert_component_gating
assert_unknown_component_is_rejected
assert_project_init_validation_contract

if run_fast_mode; then
	exit 0
fi

assert_project_flake_contract
assert_project_extension_contract
assert_human_facing_failure_contract
