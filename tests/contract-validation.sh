#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

mkdir -p "$tmpdir/contract-project"
nix run "${repo_flake_url}#robo" -- init "$tmpdir/contract-project" \
	--profile minimal \
	--robo-nix-url "path:${repo_root}" >/dev/null

(
	cd "$tmpdir/contract-project"
	nix run "${repo_flake_url}#robo" -- contract >"$tmpdir/contract.txt"
	nix run "${repo_flake_url}#robo" -- contract --json >"$tmpdir/contract.json"
	nix run "${repo_flake_url}#robo" -- check --why >"$tmpdir/why.txt"
	nix run "${repo_flake_url}#robo" -- check --why --json >"$tmpdir/why.json"
)

assert_file_contains "$tmpdir/contract.txt" 'contract: env=contract-project'
assert_file_contains "$tmpdir/contract.txt" '  schemaVersion=1'
if grep -F 'contract: schemaVersion=' "$tmpdir/contract.txt" >/dev/null; then
	echo "human contract output should not repeat the contract label for every entry" >&2
	exit 1
fi

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
assert_file_contains "$tmpdir/why.txt" 'why: profile minimal'
assert_file_contains "$tmpdir/why.txt" 'why: components'
if grep -F 'why:   base <-' "$tmpdir/why.txt" >/dev/null; then
	echo "human why output should not repeat the why label for every entry" >&2
	exit 1
fi

mkdir -p "$tmpdir/bootstrap-project/scripts"
nix run "${repo_flake_url}#robo" -- init "$tmpdir/bootstrap-project" \
	--profile minimal \
	--source-script scripts/bootstrap.sh \
	--robo-nix-url "path:${repo_root}" >/dev/null
cat >"$tmpdir/bootstrap-project/scripts/bootstrap.sh" <<'EOF'
#!/usr/bin/env bash
:
EOF

(
	cd "$tmpdir/bootstrap-project"
	nix run "${repo_flake_url}#robo" -- check --why --json >"$tmpdir/bootstrap-why.json"
)

assert_file_contains "$tmpdir/bootstrap-why.json" '"bootstrapScripts": ['
assert_file_contains "$tmpdir/bootstrap-why.json" '"name": "scripts/bootstrap.sh"'
assert_file_contains "$tmpdir/bootstrap-why.json" '"source": "manual config"'
assert_file_contains "$tmpdir/bootstrap-why.json" '"reason": "listed in provenance.sourceScripts in robo.nix"'
if grep -F '"source": "workspace inference"' "$tmpdir/bootstrap-why.json" >/dev/null; then
	echo "bootstrap sourceScripts should not be reported as workspace inference" >&2
	exit 1
fi
