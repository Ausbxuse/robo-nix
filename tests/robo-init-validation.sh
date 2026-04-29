#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

full_mode=0
if [ "${1:-}" = "--full" ]; then
	full_mode=1
	shift
fi

run_full_mode() {
	[ "$full_mode" -eq 1 ]
}

assert_listing_outputs() {
	local component_list_file="$tmpdir/components.txt"
	local profile_list_file="$tmpdir/profiles.txt"

	nix run "path:${repo_root}#robo" -- init --list-profiles >"$profile_list_file"
	assert_file_contains "$profile_list_file" "minimal"
	assert_file_contains "$profile_list_file" "isaac-ros2"

	nix run "path:${repo_root}#robo" -- init --list-components >"$component_list_file"
	assert_file_contains "$component_list_file" "base"
	assert_file_contains "$component_list_file" "ros2-jazzy"
}

assert_basic_project_init() {
	local init_output="$tmpdir/init.txt"
	local check_output="$tmpdir/check.txt"
	local robo_wrapper_check_output="$tmpdir/robo-wrapper-check.txt"

	nix run "path:${repo_root}#robo" -- init "$tmpdir/project" \
		--name project \
		--profile minimal \
		--robo-nix-url "path:${repo_root}" >"$init_output" 2>&1

	assert_file_contains "$tmpdir/project/flake.nix" 'robo-nix.url = "path:'
	assert_file_contains "$tmpdir/project/flake.lock" '"robo-nix"'
	assert_file_contains "$tmpdir/project/flake.nix" "mkProjectFlakeFromManifest"
	assert_file_contains "$tmpdir/project/robo.nix" '"base"'
	assert_file_contains "$tmpdir/project/robo.nix" '"python-uv"'
	assert_file_contains "$tmpdir/project/robo.nix" 'pythonVersion = "3.11";'
	assert_file_contains "$tmpdir/project/.python-version" "3.11"
	assert_file_contains "$tmpdir/project/pyproject.toml" 'requires-python = ">=3.11"'
	assert_file_contains "$init_output" "updated $tmpdir/project/flake.lock"
	assert_file_contains "$init_output" "(cd '$tmpdir/project' && robo check)"

	if ! run_full_mode; then
		return
	fi

	(
		cd "$tmpdir/project"
		nix run .#default -- --check >"$check_output"
	)
	assert_file_contains "$check_output" "env=project"
	assert_file_contains "$check_output" "next: run 'robo activate' to enter the environment"
	assert_file_contains "$check_output" "status=ok"

	(
		cd "$tmpdir/project"
		nix run "path:${repo_root}#robo" -- check >"$robo_wrapper_check_output"
	)
	assert_file_contains "$robo_wrapper_check_output" "robo: checked project"
	assert_file_contains "$robo_wrapper_check_output" "status"
	assert_file_contains "$robo_wrapper_check_output" "ok,"
}

assert_python_version_preflight() {
	local output="$tmpdir/python-version-preflight.txt"
	local wildcard_output="$tmpdir/python-version-wildcard.txt"

	mkdir -p "$tmpdir/python-version-project"
	cat >"$tmpdir/python-version-project/pyproject.toml" <<'EOF'
[project]
name = "python-version-project"
requires-python = "==3.12.11"
dependencies = []
EOF
	printf '3.11\n' >"$tmpdir/python-version-project/.python-version"
	cat >"$tmpdir/python-version-project/robo.nix" <<'EOF'
{
  pythonVersion = "3.12.11";
}
EOF

	(
		cd "$tmpdir/python-version-project"
		assert_command_fails_capture "$output" nix run "path:${repo_root}#robo" -- run python -c 'print("unreachable")'
	)
	assert_file_contains "$output" ".python-version is 3.11, but pyproject.toml requires Python 3.12.11"

	printf '3.12.11\n' >"$tmpdir/python-version-project/.python-version"
	sed -i 's/3.12.11/3.12/' "$tmpdir/python-version-project/robo.nix"
	(
		cd "$tmpdir/python-version-project"
		assert_command_fails_capture "$output" nix run "path:${repo_root}#robo" -- run python -c 'print("unreachable")'
	)
	assert_file_contains "$output" "robo.nix declares Python 3.12, but pyproject.toml requires Python 3.12.11"

	mkdir -p "$tmpdir/python-version-wildcard-project"
	cat >"$tmpdir/python-version-wildcard-project/pyproject.toml" <<'EOF'
[project]
name = "python-version-wildcard-project"
requires-python = "==3.10.*"
dependencies = []
EOF

	nix run "path:${repo_root}#robo" -- init "$tmpdir/python-version-wildcard-project" \
		--robo-nix-url "path:${repo_root}" >"$wildcard_output" 2>&1
	assert_file_contains "$tmpdir/python-version-wildcard-project/.python-version" "3.10"
	assert_file_contains "$tmpdir/python-version-wildcard-project/robo.nix" 'pythonVersion = "3.10";'
	assert_file_contains "$wildcard_output" "python 3.10: pyproject.toml requires-python"

	printf '3.11\n' >"$tmpdir/python-version-wildcard-project/.python-version"
	(
		cd "$tmpdir/python-version-wildcard-project"
		assert_command_fails_capture "$output" nix run "path:${repo_root}#robo" -- run python -c 'print("unreachable")'
	)
	assert_file_contains "$output" ".python-version is 3.11, but pyproject.toml requires Python 3.10"

}

assert_default_source_is_packaged_source() {
	nix run "path:${repo_root}#robo" -- init "$tmpdir/default-source-project" \
		--profile minimal >/dev/null

	assert_file_contains "$tmpdir/default-source-project/flake.nix" 'robo-nix.url = "path:/nix/store/'
}

assert_runtime_repairs_legacy_github_source() {
	local check_output="$tmpdir/legacy-source-check.txt"

	nix run "path:${repo_root}#robo" -- init "$tmpdir/legacy-source-project" \
		--profile minimal \
		--robo-nix-url "path:${repo_root}" >/dev/null
	sed -i 's|path:[^"]*|github:ausbxuse/robo-nix|' "$tmpdir/legacy-source-project/flake.nix"

	(
		cd "$tmpdir/legacy-source-project"
		nix run "path:${repo_root}#robo" -- check >"$check_output"
	)
	assert_file_contains "$tmpdir/legacy-source-project/flake.nix" 'robo-nix.url = "path:/nix/store/'
	assert_file_contains "$check_output" "robo: checked legacy-source-project"
	assert_file_contains "$check_output" "ok,"
}

assert_interactive_project_init() {
	local interactive_check_output="$tmpdir/interactive-check.txt"

	printf '\n' |
		nix run "path:${repo_root}#robo" -- init "$tmpdir/interactive-project" --interactive \
			--robo-nix-url "path:${repo_root}" >/dev/null

	test -f "$tmpdir/interactive-project/flake.nix"
	assert_file_contains "$tmpdir/interactive-project/robo.nix" '"base"'

	if ! run_full_mode; then
		return
	fi

	(
		cd "$tmpdir/interactive-project"
		nix run .#default -- --check >"$interactive_check_output"
	)
	assert_file_contains "$interactive_check_output" "env=interactive-project"
	assert_file_contains "$interactive_check_output" "next: run 'robo activate' to enter the environment"
	assert_file_contains "$interactive_check_output" "status=ok"
}

assert_robo_wrapper_runtime_flow() {
	local robo_check_output="$tmpdir/robo-check.txt"
	local robo_run_output="$tmpdir/robo-run.txt"

	nix run "path:${repo_root}#robo" -- init "$tmpdir/robo-project" \
		--name dexmate-teleop \
		--profile mujoco-sim \
		--with media,linux-headers,qt6 \
		--required-dir third_party/GMR \
		--source-script scripts/bootstrap_gmr_env.sh \
		--env "DEXMATE_GMR_SUBMODULE_PATH=\$WORKSPACE_ROOT/third_party/GMR" \
		--robo-nix-url "path:${repo_root}" >/dev/null

	assert_file_contains "$tmpdir/robo-project/flake.nix" "mkProjectFlakeFromManifest"
	assert_file_contains "$tmpdir/robo-project/robo.nix" '"media"'
	assert_file_contains "$tmpdir/robo-project/robo.nix" '"linux-headers"'
	assert_file_contains "$tmpdir/robo-project/robo.nix" '"qt6"'
	assert_file_contains "$tmpdir/robo-project/robo.nix" 'requiredDirectories = ['
	assert_file_contains "$tmpdir/robo-project/robo.nix" ". \"\$WORKSPACE_ROOT/scripts/bootstrap_gmr_env.sh\""
	assert_file_contains "$tmpdir/robo-project/robo.nix" "export DEXMATE_GMR_SUBMODULE_PATH=\"\$WORKSPACE_ROOT/third_party/GMR\""

	mkdir -p "$tmpdir/robo-project/third_party/GMR"
	mkdir -p "$tmpdir/robo-project/scripts"
	cat >"$tmpdir/robo-project/scripts/bootstrap_gmr_env.sh" <<'EOF'
#!/usr/bin/env bash
mkdir -p "$WORKSPACE_ROOT/.robo-nix"
printf 'bootstrapped\n' >"$WORKSPACE_ROOT/.robo-nix/bootstrap-stamp"
EOF

	(
		cd "$tmpdir/robo-project"
		nix run .#default -- --check >"$robo_check_output"
	)
	assert_file_contains "$robo_check_output" "env=dexmate-teleop"
	assert_file_contains "$robo_check_output" "status=ok"

	(
		cd "$tmpdir/robo-project"
		nix run "path:${repo_root}#robo" -- run python -c 'from pathlib import Path; assert Path(".robo-nix/bootstrap-stamp").is_file()' >"$robo_run_output"
	)
	test ! -s "$robo_run_output"
}

assert_runtime_probe_inference() {
	local probed_check_output="$tmpdir/probed-check.txt"
	local probed_init_output="$tmpdir/probed-init.txt"
	local probed_why_output="$tmpdir/probed-why.json"
	local probed_contract_output="$tmpdir/probed-contract.json"

	mkdir -p "$tmpdir/probed-project"
	mkdir -p "$tmpdir/probed-project/scripts" "$tmpdir/probed-project/third_party/local-sdk"
	cat >"$tmpdir/probed-project/pyproject.toml" <<'EOF'
[project]
name = "probed-project"
requires-python = "==3.12.11"
dependencies = [
  "mujoco>=3.3",
  "opencv-python",
  "av",
  "isaacsim[all,extscache]==4.5.0",
  "pyside6",
]
EOF
	cat >"$tmpdir/probed-project/scripts/bootstrap_local_sdk.sh" <<'EOF'
#!/usr/bin/env bash
sdk_dir="${PROJECT_SDK_DIR:-$PWD/third_party/local-sdk}"
source_checkout_ready "$sdk_dir" setup.py src/bindings.cpp CMakeLists.txt
EOF

	nix run "path:${repo_root}#robo" -- init "$tmpdir/probed-project" \
		--robo-nix-url "path:${repo_root}" >"$probed_init_output" 2>&1
	nix run "path:${repo_root}#robo" -- init "$tmpdir/probed-project" \
		--robo-nix-url "path:${repo_root}" >/dev/null

	assert_file_contains "$tmpdir/probed-project/robo.nix" 'envName = "probed-project";'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'schemaVersion = 1;'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"mujoco"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"x11-gl"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"media"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"cuda-toolkit"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"isaac-sim"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"qt6"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'provenance = {'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'componentReasons = ['
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'source = "pyproject inference";'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'suggestions = ['
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"third_party/local-sdk/setup.py"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" '"third_party/local-sdk/src/bindings.cpp"'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'pythonVersion = "3.12.11";'
	assert_file_contains "$tmpdir/probed-project/robo.nix" 'profile = "minimal";'
	assert_file_contains "$tmpdir/probed-project/.python-version" "3.12.11"
	assert_file_contains "$tmpdir/probed-project/pyproject.toml" "opencv-python"
	if [ "$(grep -F -c "OpenCV wheels commonly need graphics and media runtime libraries" "$probed_init_output")" -ne 1 ]; then
		echo "OpenCV inference note should appear once in init output" >&2
		exit 1
	fi
	if grep -F "requiredFiles" "$tmpdir/probed-project/robo.nix" >/dev/null; then
		echo "low-confidence source file inference became a hard requirement" >&2
		exit 1
	fi

	(
		cd "$tmpdir/probed-project"
		nix run "path:${repo_root}#robo" -- check --why --json >"$probed_why_output"
		nix run "path:${repo_root}#robo" -- contract --json >"$probed_contract_output"
	)
	assert_file_contains "$probed_why_output" '"profile": "minimal"'
	assert_file_contains "$probed_why_output" '"name": "base"'
	assert_file_contains "$probed_why_output" '"source": "profile"'
	assert_file_contains "$probed_why_output" '"name": "mujoco"'
	assert_file_contains "$probed_why_output" '"name": "isaac-sim"'
	assert_file_contains "$probed_why_output" '"source": "pyproject inference"'
	assert_file_contains "$probed_why_output" '"suggestions": ['
	assert_file_contains "$probed_contract_output" '"envName": "probed-project"'
	assert_file_contains "$probed_contract_output" '"schemaVersion": "1"'
	assert_file_contains "$probed_contract_output" '"defaultDerivation":'
	assert_file_contains "$probed_contract_output" '"components": ['

	if ! run_full_mode; then
		return
	fi

	mkdir -p "$tmpdir/probed-project/third_party/local-sdk/src"
	printf 'from setuptools import setup\n' >"$tmpdir/probed-project/third_party/local-sdk/setup.py"
	printf 'int main() { return 0; }\n' >"$tmpdir/probed-project/third_party/local-sdk/src/bindings.cpp"
	printf 'cmake_minimum_required(VERSION 3.20)\n' >"$tmpdir/probed-project/third_party/local-sdk/CMakeLists.txt"
	(
		cd "$tmpdir/probed-project"
		nix run .#default -- --check >"$probed_check_output"
	)
	assert_file_contains "$probed_check_output" "env=probed-project"
	assert_file_contains "$probed_check_output" "suggestion: check whether third_party/local-sdk/setup.py should be required for this project"
	assert_file_contains "$probed_check_output" "status=ok"
}

assert_cuda_workspace_component_suggestion() {
	local cuda_output="$tmpdir/cuda-suggestion.txt"
	local interactive_output="$tmpdir/cuda-interactive.txt"

	mkdir -p "$tmpdir/cuda-suggestion-project/src"
	cat >"$tmpdir/cuda-suggestion-project/pyproject.toml" <<'EOF'
[project]
name = "cuda-suggestion-project"
requires-python = "==3.11.*"
dependencies = []
EOF
	printf '__global__ void kernel() {}\n' >"$tmpdir/cuda-suggestion-project/src/kernel.cu"

	nix run "path:${repo_root}#robo" -- init "$tmpdir/cuda-suggestion-project" \
		--robo-nix-url "path:${repo_root}" >"$cuda_output" 2>&1
	assert_file_contains "$cuda_output" "component cuda-toolkit: workspace contains CUDA extension markers; src/kernel.cu: CUDA source file"
	assert_file_contains "$tmpdir/cuda-suggestion-project/robo.nix" "componentSuggestions = ["
	assert_file_contains "$tmpdir/cuda-suggestion-project/robo.nix" 'name = "cuda-toolkit";'
	if grep -F '    "cuda-toolkit"' "$tmpdir/cuda-suggestion-project/robo.nix" >/dev/null; then
		echo "non-interactive CUDA marker should suggest cuda-toolkit without adding it" >&2
		exit 1
	fi

	mkdir -p "$tmpdir/cuda-interactive-project"
	cat >"$tmpdir/cuda-interactive-project/pyproject.toml" <<'EOF'
[project]
name = "cuda-interactive-project"
requires-python = "==3.11.*"
dependencies = []
EOF
	cat >"$tmpdir/cuda-interactive-project/setup.py" <<'EOF'
from torch.utils.cpp_extension import CUDAExtension
EOF

	nix run "path:${repo_root}#robo" -- init "$tmpdir/cuda-interactive-project" \
		--interactive \
		--robo-nix-url "path:${repo_root}" >"$interactive_output" 2>&1
	assert_file_contains "$tmpdir/cuda-interactive-project/robo.nix" '"cuda-toolkit"'
	assert_file_contains "$tmpdir/cuda-interactive-project/robo.nix" 'source = "interactive workspace inference";'
	assert_file_contains "$tmpdir/cuda-interactive-project/robo.nix" "setup.py: CUDA build marker"
	if grep -F "componentSuggestions" "$tmpdir/cuda-interactive-project/robo.nix" >/dev/null; then
		echo "accepted interactive CUDA suggestion should not remain pending" >&2
		exit 1
	fi
}

assert_check_reports_stale_runtime_contract() {
	local stale_check_output="$tmpdir/stale-check.txt"

	mkdir -p "$tmpdir/stale-project"
	cat >"$tmpdir/stale-project/pyproject.toml" <<'EOF'
[project]
name = "stale-project"
requires-python = "==3.12.11"
dependencies = [
  "pyav",
]
EOF

	nix run "path:${repo_root}#robo" -- init "$tmpdir/stale-project" \
		--robo-nix-url "path:${repo_root}" >/dev/null
	sed -i '/"media"/d' "$tmpdir/stale-project/robo.nix"

	(
		cd "$tmpdir/stale-project"
		nix run "path:${repo_root}#robo" -- check >"$stale_check_output"
	)
	assert_file_contains "$stale_check_output" "runtime components may be incomplete"
	assert_file_contains "$stale_check_output" "missing: media"
	assert_file_contains "$stale_check_output" "ok,"
}

assert_tricky_package_runtime_inference() {
	local tricky_why_output="$tmpdir/tricky-why.json"
	local tricky_contract_output="$tmpdir/tricky-contract.json"

	mkdir -p "$tmpdir/tricky-project"
	cat >"$tmpdir/tricky-project/pyproject.toml" <<'EOF'
[project]
name = "tricky-project"
requires-python = "==3.12.11"
dependencies = [
  "cuda-python",
  "cupy-cuda12x",
  "pyav",
  "lerobot",
  "torchvision",
  "pytorch3d",
  "torch3d",
  "flash-attn",
  "evdev",
]
EOF

	nix run "path:${repo_root}#robo" -- init "$tmpdir/tricky-project" \
		--robo-nix-url "path:${repo_root}" >/dev/null

	assert_file_contains "$tmpdir/tricky-project/robo.nix" 'envName = "tricky-project";'
	assert_file_contains "$tmpdir/tricky-project/robo.nix" '"media"'
	assert_file_contains "$tmpdir/tricky-project/robo.nix" '"x11-gl"'
	assert_file_contains "$tmpdir/tricky-project/robo.nix" '"cuda-toolkit"'
	assert_file_contains "$tmpdir/tricky-project/robo.nix" '"linux-headers"'
	assert_file_contains "$tmpdir/tricky-project/robo.nix" 'source = "pyproject inference";'

	(
		cd "$tmpdir/tricky-project"
		nix run "path:${repo_root}#robo" -- check --why --json >"$tricky_why_output"
		nix run "path:${repo_root}#robo" -- contract --json >"$tricky_contract_output"
	)
	assert_file_contains "$tricky_why_output" '"envName": "tricky-project"'
	assert_file_contains "$tricky_why_output" '"name": "media"'
	assert_file_contains "$tricky_why_output" '"name": "x11-gl"'
	assert_file_contains "$tricky_why_output" '"name": "cuda-toolkit"'
	assert_file_contains "$tricky_why_output" '"name": "linux-headers"'
	assert_file_contains "$tricky_why_output" '"source": "pyproject inference"'
	assert_file_contains "$tricky_contract_output" '"cuda-toolkit"'
	assert_file_contains "$tricky_contract_output" '"linux-headers"'
}

assert_ros_project_init_full() {
	local ros_check_output="$tmpdir/ros-check.txt"

	nix run "path:${repo_root}#robo" -- init "$tmpdir/ros-project" \
		--name ros-project \
		--profile ros2-workspace \
		--robo-nix-url "path:${repo_root}" >/dev/null

	assert_file_contains "$tmpdir/ros-project/robo.nix" '"ros2-jazzy"'
	assert_file_contains "$tmpdir/ros-project/robo.nix" '"ros-workspace"'
	test -d "$tmpdir/ros-project/ros_ws/src"

	(
		cd "$tmpdir/ros-project"
		nix run .#default -- --check >"$ros_check_output"
	)
	assert_file_contains "$ros_check_output" "env=ros-project"
	assert_file_contains "$ros_check_output" "status=ok"
}

assert_stdout_generation() {
	local generated_flake="$tmpdir/generated-flake.nix"

	nix run "path:${repo_root}#robo" -- init --stdout \
		--name project \
		--components base,python-uv,native-build \
		--python-version 3.11 \
		--systems x86_64-linux \
		--robo-nix-url "path:${repo_root}" >"$generated_flake"

	assert_file_contains "$generated_flake" "mkProjectFlakeFromManifest"
}

assert_listing_outputs
assert_default_source_is_packaged_source
assert_runtime_repairs_legacy_github_source
assert_basic_project_init
assert_python_version_preflight
assert_interactive_project_init
assert_runtime_probe_inference
assert_cuda_workspace_component_suggestion
assert_check_reports_stale_runtime_contract
assert_tricky_package_runtime_inference
assert_stdout_generation

if run_full_mode; then
	assert_ros_project_init_full
	assert_robo_wrapper_runtime_flow
fi
