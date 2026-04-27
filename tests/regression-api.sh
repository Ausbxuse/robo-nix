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
      assert builtins.any (rule: builtins.elem "xrobot" rule.nameContains && builtins.elem "qt6" rule.components) inference.workspaceDirectoryRules;
      assert builtins.elem "bootstrap_" inference.scriptDiscovery.prefixes;
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
      assert builtins.hasAttr "cuda-doctor" flake.apps.x86_64-linux;
      assert builtins.hasAttr "robo" flake.apps.x86_64-linux;
      assert builtins.hasAttr "repo-fmt" flake.packages.x86_64-linux;
      assert builtins.hasAttr "repo-lint" flake.packages.x86_64-linux;
      assert builtins.hasAttr "repo-profile" flake.packages.x86_64-linux;
      assert builtins.hasAttr "cuda-doctor" flake.packages.x86_64-linux;
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
      assert builtins.hasAttr "gpu-required-cuda-doctor-contract" flake.checks.x86_64-linux;
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
	tmpdir="$(mktemp_dir)"
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
        "x86_64-linux"
      ];
      workspaceRoot = ".";
    };
}
EOF

	local expr
	expr='
    let
      flake = builtins.getFlake "'"path:${tmpdir}"'";
    in
      assert builtins.hasAttr "x86_64-linux" flake.apps;
      assert !(builtins.hasAttr "aarch64-darwin" flake.apps);
      assert builtins.hasAttr "default" flake.apps.x86_64-linux;
      assert builtins.hasAttr "contract" flake.apps.x86_64-linux;
      true
  '
	assert_expr "$expr"

	local config_file
	local doctor_file
	config_file="$tmpdir/contract.config"
	doctor_file="$tmpdir/contract.doctor"
	nix run "$tmpdir#default" -- --print-config >"$config_file"
	nix run "$tmpdir#default" -- --doctor >"$doctor_file"
	grep -F "env=contract" "$config_file" >/dev/null
	grep -F "python=3.11" "$config_file" >/dev/null
	grep -F "component=base" "$config_file" >/dev/null
	grep -F "component=python-uv" "$config_file" >/dev/null
	grep -F "doctor: env=contract" "$doctor_file" >/dev/null
	grep -F "doctor: next: run 'robo develop' to enter the environment" "$doctor_file" >/dev/null
	grep -F "doctor: status=ok" "$doctor_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_project_extension_contract() {
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
      doctor = ''
        doctor_ok "project extension doctor ran"
      '';
      requiredDirectories = [
        "third_party/example"
      ];
      supportedSystems = [
        "x86_64-linux"
      ];
    };
}
EOF

	local doctor_file
	local dryrun_file
	doctor_file="$tmpdir/extended.doctor"
	dryrun_file="$tmpdir/extended.dryrun"

	mkdir -p "$tmpdir/third_party/example"
	(
		cd "$tmpdir"
		nix run .#default -- --doctor >"$doctor_file"
		nix run .#default -- --dry-run >"$dryrun_file"
	)
	grep -F "doctor: ok: project extension doctor ran" "$doctor_file" >/dev/null
	grep -F "validated extended with Python 3.11" "$dryrun_file" >/dev/null

	trap - RETURN
	cleanup_dir "$tmpdir"
}

assert_human_facing_failure_contract() {
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
      envName = "failing-contract";
      components = [
        "base"
        "python-uv"
        "native-build"
        "ros-workspace"
      ];
      pythonVersion = "3.11";
      supportedSystems = [
        "x86_64-linux"
      ];
      workspaceRoot = ".";
    };
}
EOF

	local doctor_file
	local dryrun_file
	doctor_file="$tmpdir/failure.doctor"
	dryrun_file="$tmpdir/failure.dryrun"

	assert_command_fails_capture "$doctor_file" nix run "$tmpdir#default" -- --doctor
	grep -F "doctor: error: missing workspace directory: ros_ws/src" "$doctor_file" >/dev/null
	grep -F "doctor: hint: create " "$doctor_file" >/dev/null
	grep -F "doctor: next: fix the issues above and rerun 'robo doctor'" "$doctor_file" >/dev/null
	grep -F "doctor: status=error" "$doctor_file" >/dev/null

	assert_command_fails_capture "$dryrun_file" nix run "$tmpdir#default" -- --dry-run
	grep -F "bootstrap error: missing required directory:" "$dryrun_file" >/dev/null
	grep -F "doctor: hint: run 'robo doctor' for a full setup report" "$dryrun_file" >/dev/null

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
