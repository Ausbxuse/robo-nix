#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

log_dir="$(mktemp_dir)"
trap 'cleanup_dir "$log_dir"' EXIT

pids=()
names=()
logs=()

start_check() {
	local name="$1"
	shift

	local log_file="$log_dir/${name}.log"
	printf 'start: %s\n' "$name"
	("$@") >"$log_file" 2>&1 &
	pids+=("$!")
	names+=("$name")
	logs+=("$log_file")
}

start_check fmt nix run .#repo-fmt -- --check
start_check lint nix run .#repo-lint
start_check flake-check nix flake check --print-build-logs
start_check regression bash tests/regression-api.sh
start_check profile bash tests/profile-validation.sh
start_check fixtures bash tests/fixture-validation.sh
start_check robo-init-full bash tests/robo-init-validation.sh --full
start_check vendors bash tests/vendor-validation.sh
start_check contract bash tests/contract-validation.sh
start_check output-consistency bash tests/output-consistency.sh

failed=0

for index in "${!pids[@]}"; do
	if wait "${pids[$index]}"; then
		printf 'ok: %s\n' "${names[$index]}"
	else
		failed=1
		printf 'failed: %s\n' "${names[$index]}" >&2
		printf -- '--- %s log ---\n' "${names[$index]}" >&2
		cat "${logs[$index]}" >&2
	fi
done

if [ "$failed" -ne 0 ]; then
	exit 1
fi
