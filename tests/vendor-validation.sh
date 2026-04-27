#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

list_output="$tmpdir/vendor-list.txt"
empty_doctor_output="$tmpdir/vendor-empty-doctor.txt"
gmr_doctor_output="$tmpdir/vendor-gmr-doctor.txt"
gmr_add_output="$tmpdir/vendor-gmr-add.txt"
gmr_export_output="$tmpdir/vendor-gmr-export.nix"
gmr_bootstrap_output="$tmpdir/vendor-gmr-bootstrap.txt"

nix run "path:${repo_root}#robo" -- vendor list >"$list_output"
assert_file_contains "$list_output" "dexmate-gmr"
assert_file_contains "$list_output" "third_party/GMR"
assert_file_contains "$list_output" "dexmate-xrobot-pc-service"
assert_file_contains "$list_output" "dexmate-vega-navigation-stack"

mkdir -p "$tmpdir/empty-project"
(
	cd "$tmpdir/empty-project"
	nix run "path:${repo_root}#robo" -- vendor doctor >"$empty_doctor_output"
)
assert_file_contains "$empty_doctor_output" "vendor: status=ok detected=0"

mkdir -p "$tmpdir/gmr-project/third_party/GMR"
mkdir -p "$tmpdir/gmr-project/third_party/vendor-patches"
mkdir -p "$tmpdir/gmr-project/scripts"
: >"$tmpdir/gmr-project/third_party/GMR/setup.py"
: >"$tmpdir/gmr-project/third_party/vendor-patches/gmr-dexmate-vega1-addon.patch"
: >"$tmpdir/gmr-project/scripts/apply_vendor_patches.sh"
: >"$tmpdir/gmr-project/scripts/bootstrap_gmr_env.sh"
(
	cd "$tmpdir/gmr-project"
	nix run "path:${repo_root}#robo" -- vendor doctor >"$gmr_doctor_output"
	nix run "path:${repo_root}#robo" -- vendor add third_party/GMR >"$gmr_add_output"
	nix run "path:${repo_root}#robo" -- vendor export dexmate-gmr >"$gmr_export_output"
	nix run "path:${repo_root}#robo" -- vendor bootstrap >"$gmr_bootstrap_output"
)
assert_file_contains "$gmr_doctor_output" "vendor: ok: dexmate-gmr source present at third_party/GMR"
assert_file_contains "$gmr_doctor_output" "vendor: info: dexmate-gmr suggests components: mujoco,native-build"
assert_file_contains "$gmr_doctor_output" "vendor: status=ok detected=1"
assert_file_contains "$gmr_add_output" "vendor: ok: third_party/GMR matches curated module dexmate-gmr"
assert_file_contains "$gmr_add_output" "vendor: hint: add components to robo.nix: mujoco,native-build"
assert_file_contains "$gmr_export_output" "dexmate-gmr = {"
assert_file_contains "$gmr_export_output" 'installPath = "third_party/GMR";'
assert_file_contains "$gmr_export_output" 'sourceUrl = null;'
assert_file_contains "$gmr_bootstrap_output" "vendor: status=ok scripts=2"
