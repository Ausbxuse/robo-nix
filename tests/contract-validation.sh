#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

mkdir -p "$tmpdir/contract-project"
nix run "path:${repo_root}#robo" -- init "$tmpdir/contract-project" \
	--profile minimal \
	--robo-nix-url "path:${repo_root}" >/dev/null

(
	cd "$tmpdir/contract-project"
	nix run "path:${repo_root}#robo" -- contract --json >"$tmpdir/contract.json"
	nix run "path:${repo_root}#robo" -- check --why --json >"$tmpdir/why.json"
)

assert_file_contains "$tmpdir/contract.json" '"envName": "contract-project"'
assert_file_contains "$tmpdir/contract.json" '"schemaVersion": "1"'
assert_file_contains "$tmpdir/contract.json" '"system": "x86_64-linux"'
assert_file_contains "$tmpdir/contract.json" '"defaultDerivation":'
assert_file_contains "$tmpdir/contract.json" '"flakeLockPresent":'
assert_file_contains "$tmpdir/contract.json" '"components": ['
assert_file_contains "$tmpdir/contract.json" '"source": "profile"'

assert_file_contains "$tmpdir/why.json" '"profile": "minimal"'
assert_file_contains "$tmpdir/why.json" '"components": ['
assert_file_contains "$tmpdir/why.json" '"removeHint":'
