#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

output_file="$tmpdir/repo-profile.txt"

run_profile() {
	nix run "${repo_flake_url}#repo-profile" >"$output_file" 2>&1
}

if ! run_profile; then
	cat "$output_file" >&2
	printf 'retrying repo-profile once after initial failure\n' >&2
	sleep 1
	if ! run_profile; then
		cat "$output_file" >&2
		exit 1
	fi
fi

grep -F "profiling robo-nix at path:" "$output_file" >/dev/null
grep -F "#apps.x86_64-linux.default.program" "$output_file" >/dev/null
grep -F "nix flake show path:" "$output_file" >/dev/null
