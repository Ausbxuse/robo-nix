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
assert_file_contains "$tmpdir/glibc.out" "error[python.glibc-abi-mix]: Host Python/glibc is mixing with Nix native libraries"
assert_file_contains "$tmpdir/glibc.out" "evidence"
assert_file_contains "$tmpdir/glibc.out" "robo shell"

printf '%s\n' "GLFWError: (65542) b'EGL: Failed to get EGL display: Success'" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl.out"
assert_file_contains "$tmpdir/egl.out" "error[graphics.egl-context]: EGL/OpenGL context creation failed"
assert_file_contains "$tmpdir/egl.out" "robo check graphics --verbose"

printf '%s\n' "Failed to get EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-short.out"
assert_file_contains "$tmpdir/egl-short.out" "error[graphics.egl-context]: EGL/OpenGL context creation failed"
assert_file_contains "$tmpdir/egl-short.out" "  Failed to get EGL display"
assert_file_not_contains "$tmpdir/egl-short.out" "gladLoadGL error"

printf '%s\n' "Failed EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-shorter.out"
assert_file_contains "$tmpdir/egl-shorter.out" "error[graphics.egl-context]: EGL/OpenGL context creation failed"
assert_file_contains "$tmpdir/egl-shorter.out" "  Failed EGL display"

printf '%s\n' "EGL display" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/egl-search.out"
assert_file_contains "$tmpdir/egl-search.out" "no diagnosis matched"
assert_file_contains "$tmpdir/egl-search.out" "graphics.egl-context"
assert_file_not_contains "$tmpdir/egl-search.out" "agent prompt"
assert_file_not_contains "$tmpdir/egl-search.out" "handoff commands"

printf '%s\n' "Could not find Qt6Config.cmake" |
	"${robo_cli[@]}" --no-color diagnose --json - >"$tmpdir/qt.json"
assert_file_contains "$tmpdir/qt.json" '"schema": "robo.diagnosis.v1"'
assert_file_contains "$tmpdir/qt.json" '"id": "native.qt6-cmake-missing"'
assert_file_contains "$tmpdir/qt.json" '"suggestions": []'
assert_file_not_contains "$tmpdir/qt.json" "agent_handoff"
assert_file_not_contains "$tmpdir/qt.json" '"confidence"'

printf '%s\n' "warn: Python virtualenv contains native build tool shims: cmake" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/native-shim.out"
assert_file_contains "$tmpdir/native-shim.out" "warning[native.python-build-tool-shim]: Python-owned native build tool shim is crossing the ABI boundary"
assert_file_contains "$tmpdir/native-shim.out" "Python virtualenv contains native build tool shims"
assert_file_contains "$tmpdir/native-shim.out" "https://ausbxuse.github.io/robo-nix/users/troubleshooting#native-build-tool-shims-in-venv"

printf '%s\n' "warn[native.python-build-tool-shim]: Python virtualenv contains native build tool shims: cmake" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/native-shim-id.out"
assert_file_contains "$tmpdir/native-shim-id.out" "warning[native.python-build-tool-shim]: Python-owned native build tool shim is crossing the ABI boundary"
assert_file_contains "$tmpdir/native-shim-id.out" "  native.python-build-tool-shim"

printf '%s\n' "error: Python virtualenv was created outside robo-nix" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/host-venv.out"
assert_file_contains "$tmpdir/host-venv.out" "error[python.env-host-owned]: Python virtualenv was created outside robo-nix"
assert_file_contains "$tmpdir/host-venv.out" "uv venv --python"

printf '%s\n' "! runtime: runtime components may be incomplete" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/runtime-components.out"
assert_file_contains "$tmpdir/runtime-components.out" "warning[runtime.components-incomplete]: Runtime components may be incomplete"
assert_file_contains "$tmpdir/runtime-components.out" "robo init . --force"

printf '%s\n' "error: CUDA native build surface is incomplete" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/cuda-toolkit.out"
assert_file_contains "$tmpdir/cuda-toolkit.out" "error[cuda.toolkit-not-visible]: CUDA toolkit is not visible in the runtime"
assert_file_contains "$tmpdir/cuda-toolkit.out" "robo check --deep"
assert_file_contains "$tmpdir/cuda-toolkit.out" "https://ausbxuse.github.io/robo-nix/users/troubleshooting#cuda-toolkit-not-visible"

printf '%s\n' "warn: TorchCodec import failed" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/torchcodec.out"
assert_file_contains "$tmpdir/torchcodec.out" "warning[media.ffmpeg-runtime-missing]: FFmpeg media runtime is missing or incomplete"
assert_file_contains "$tmpdir/torchcodec.out" "add media to robo.nix components"
assert_file_contains "$tmpdir/torchcodec.out" "https://ausbxuse.github.io/robo-nix/users/troubleshooting#ffmpeg-media-runtime-missing"

printf '%s\n' "EGL display" |
	"${robo_cli[@]}" --no-color diagnose --json - >"$tmpdir/egl-search.json"
assert_file_contains "$tmpdir/egl-search.json" '"suggestions": ['
assert_file_contains "$tmpdir/egl-search.json" '"id": "graphics.egl-context"'
assert_file_not_contains "$tmpdir/egl-search.json" "agent_handoff"
assert_file_not_contains "$tmpdir/egl-search.json" '"confidence"'

printf '%s\n' "plain pytest assertion failure" |
	"${robo_cli[@]}" --no-color diagnose - >"$tmpdir/unknown.out"
assert_file_contains "$tmpdir/unknown.out" "no diagnosis matched"
