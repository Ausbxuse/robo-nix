#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT
robo_bin="$(nix build --no-link --print-out-paths "${repo_flake_url}#robo")/bin/robo"
robo_cli=("$robo_bin")

glibc_log="$tmpdir/glibc.log"
cat >"$glibc_log" <<'LOG'
ImportError: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.38' not found
(required by /nix/store/41ym1jm1b7j3rhglk82gwg9jml26z1km-gcc-14.3.0-lib/lib/libstdc++.so.6)
LOG

"${robo_cli[@]}" --no-color diagnose "$glibc_log" >"$tmpdir/glibc.out"
assert_file_contains "$tmpdir/glibc.out" "diagnosis: Host Python/glibc is mixing with Nix native libraries"
assert_file_contains "$tmpdir/glibc.out" "id: python.glibc-abi-mix"
assert_file_contains "$tmpdir/glibc.out" "robo shell"

printf '%s\n' "GLFWError: (65542) b'EGL: Failed to get EGL display: Success'" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl.out"
assert_file_contains "$tmpdir/egl.out" "id: graphics.egl-context"
assert_file_contains "$tmpdir/egl.out" "robo check graphics --verbose"

printf '%s\n' "Failed to get EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-short.out"
assert_file_contains "$tmpdir/egl-short.out" "id: graphics.egl-context"
assert_file_contains "$tmpdir/egl-short.out" "  Failed to get EGL display"
if grep -F "gladLoadGL error" "$tmpdir/egl-short.out" >/dev/null; then
	echo "diagnose should print actual matched evidence only" >&2
	exit 1
fi

printf '%s\n' "Failed EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-shorter.out"
assert_file_contains "$tmpdir/egl-shorter.out" "id: graphics.egl-context"
assert_file_contains "$tmpdir/egl-shorter.out" "  Failed EGL display"

printf '%s\n' "EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-search.out"
assert_file_contains "$tmpdir/egl-search.out" "no diagnosis: not enough evidence"
assert_file_contains "$tmpdir/egl-search.out" "graphics.egl-context"
assert_file_contains "$tmpdir/egl-search.out" "agent handoff:"
assert_file_contains "$tmpdir/egl-search.out" "robo diagnose --json /tmp/robo-error.log"

printf '%s\n' "Could not find Qt6Config.cmake" |
	"${robo_cli[@]}" --no-color diagnose --json - >"$tmpdir/qt.json"
assert_file_contains "$tmpdir/qt.json" '"schema": "robo.diagnosis.v1"'
assert_file_contains "$tmpdir/qt.json" '"id": "native.qt6-cmake-missing"'
assert_file_contains "$tmpdir/qt.json" '"suggestions": []'
assert_file_contains "$tmpdir/qt.json" '"agent_handoff": null'

printf '%s\n' "EGL display" |
	"${robo_cli[@]}" --no-color diagnose --json - >"$tmpdir/egl-search.json"
assert_file_contains "$tmpdir/egl-search.json" '"suggestions": ['
assert_file_contains "$tmpdir/egl-search.json" '"id": "graphics.egl-context"'
assert_file_contains "$tmpdir/egl-search.json" '"agent_handoff": {'

printf '%s\n' "plain pytest assertion failure" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/unknown.out"
assert_file_contains "$tmpdir/unknown.out" "no known runtime failure matched"
