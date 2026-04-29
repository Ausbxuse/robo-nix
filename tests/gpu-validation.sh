#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

require_command() {
	local command_name="$1"
	if ! command -v "$command_name" >/dev/null 2>&1; then
		echo "missing required command: $command_name" >&2
		exit 1
	fi
}

require_command nvidia-smi
require_command nix

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

gpu_config_file="$tmpdir/gpu-config.txt"

nvidia-smi >/dev/null
nix run "path:${repo_root}#cuda-check" >/dev/null
nix run "path:${repo_root}#gpu-learning" -- --print-config >"$gpu_config_file"
grep -F "component=cuda-toolkit" "$gpu_config_file" >/dev/null
grep -F "python=3.11" "$gpu_config_file" >/dev/null
nix run "path:${repo_root}#gpu-learning" -- --dry-run >/dev/null
# shellcheck disable=SC2016
nix develop "path:${repo_root}#gpu-learning" --command bash -lc 'test -n "${CUDA_HOME:-}" && test -n "${CUDA_PATH:-}"'
