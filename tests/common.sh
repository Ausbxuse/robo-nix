#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mktemp_dir() {
	local dir
	dir="$(mktemp -d)"
	physical_path "$dir"
}

physical_path() {
	local path="$1"
	local dir
	local base

	if [ -d "$path" ]; then
		(cd "$path" && pwd -P)
	else
		dir="$(dirname "$path")"
		base="$(basename "$path")"
		printf '%s/%s\n' "$(cd "$dir" && pwd -P)" "$base"
	fi
}

cleanup_dir() {
	local dir="$1"
	rm -rf "$dir"
}

current_nix_system() {
	nix eval --impure --raw --expr builtins.currentSystem
}

rewrite_robo_nix_input() {
	local flake_file="$1"
	sed -i "s|github:ausbxuse/robo-nix|path:${repo_root}|" "$flake_file"
}

copy_fixture_to_tmp() {
	local fixture_name="$1"
	local tmpdir="$2"

	cp -R "$repo_root/tests/fixtures/${fixture_name}/." "$tmpdir/"
	chmod -R u+w "$tmpdir"
}

assert_file_contains() {
	local file="$1"
	local expected="$2"
	if ! grep -F "$expected" "$file" >/dev/null; then
		printf 'expected %s to contain: %s\n' "$file" "$expected" >&2
		printf -- '--- %s ---\n' "$file" >&2
		cat "$file" >&2
		printf -- '--- end %s ---\n' "$file" >&2
		exit 1
	fi
}

assert_command_fails() {
	if "$@"; then
		echo "expected command to fail: $*" >&2
		exit 1
	fi
}

assert_command_fails_capture() {
	local output_file="$1"
	shift
	if "$@" >"$output_file" 2>&1; then
		echo "expected command to fail: $*" >&2
		exit 1
	fi
}
