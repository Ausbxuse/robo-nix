#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

run_fixture_test() {
	local fixture_name="$1"
	local tmpdir

	tmpdir="$(mktemp_dir)"
	copy_fixture_to_tmp "$fixture_name" "$tmpdir"

	(
		cd "$tmpdir"
		rewrite_robo_nix_input flake.nix
		nix flake check . >/dev/null
	)

	cleanup_dir "$tmpdir"
}

run_fixture_test "minimal"
run_fixture_test "ros2-jazzy"
run_fixture_test "cuda-profiles"
