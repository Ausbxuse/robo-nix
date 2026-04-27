#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

output_file="$tmpdir/repo-profile.txt"

nix run "path:${repo_root}#repo-profile" >"$output_file"

grep -F "profiling robo-nix at path:" "$output_file" >/dev/null
grep -F "#apps.x86_64-linux.default.program" "$output_file" >/dev/null
grep -F "nix flake show path:" "$output_file" >/dev/null
