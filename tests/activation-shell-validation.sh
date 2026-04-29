#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

robo="$(nix build "path:${repo_root}#robo" --no-link --print-out-paths)/bin/robo"

project="$tmpdir/project"
mkdir -p "$project"
cat >"$project/flake.nix" <<EOF
{
  inputs.robo-nix.url = "path:${repo_root}";
  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}
EOF
cat >"$project/robo.nix" <<'EOF'
{
  envName = "activation-shell-project";
  schemaVersion = 1;
  pythonVersion = "3.11";
  components = [
    "base"
    "python-uv"
  ];
}
EOF
printf '3.11\n' >"$project/.python-version"
cat >"$project/pyproject.toml" <<'EOF'
[project]
name = "activation-shell-project"
requires-python = "==3.11.*"
dependencies = []
EOF

outside_status="$tmpdir/outside-status.txt"
(
	cd "$project"
	"$robo" status >"$outside_status"
)
assert_file_contains "$outside_status" "active=no"
assert_file_contains "$outside_status" "robo activate"

make_fake_shell() {
	local name="$1"
	local path="$tmpdir/$name"
	cat >"$path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf 'fake-shell=%s\n' "$0"
printf 'args=%s\n' "$*"
printf 'active=%s\n' "${ROBO_NIX_ACTIVE:-}"
printf 'env=%s\n' "${ROBO_NIX_ENV_NAME:-}"
printf 'shell=%s\n' "${SHELL:-}"
printf 'prompt-prefix=%s\n' "${ROBO_NIX_PROMPT_PREFIX:-}"

test "${ROBO_NIX_ACTIVE:-}" = "1"
test "${ROBO_NIX_ENV_NAME:-}" = "activation-shell-project"
test "${SHELL:-}" = "${EXPECTED_SHELL:?}"
test "${ROBO_NIX_PROMPT_PREFIX:-}" = "<activation-shell-project> "
command -v robo >/dev/null
robo status >/dev/null
test "$#" -eq 1
test "$1" = "-i"
EOF
	chmod +x "$path"
	printf '%s\n' "$path"
}

for shell_name in sh bash zsh fish nu; do
	fake_shell="$(make_fake_shell "$shell_name")"
	output="$tmpdir/activate-$shell_name.txt"
	(
		cd "$project"
		ROBO_NIX_SHELL="$fake_shell" EXPECTED_SHELL="$fake_shell" \
			"$robo" activate >"$output"
	)
	assert_file_contains "$output" "active=1"
	assert_file_contains "$output" "env=activation-shell-project"
	assert_file_contains "$output" "shell=$fake_shell"
	assert_file_contains "$output" "prompt-prefix=<activation-shell-project> "
done

active_status="$tmpdir/active-status.txt"
(
	cd "$project"
	ROBO_NIX_ACTIVE=1 \
		ROBO_NIX_ENV_NAME=activation-shell-project \
		ROBO_NIX_PYTHON_VERSION=3.11 \
		WORKSPACE_ROOT="$project" \
		ROBO_NIX_PROMPT_PREFIX="<activation-shell-project> " \
		"$robo" status >"$active_status"
)
assert_file_contains "$active_status" "active=yes"
assert_file_contains "$active_status" "status"
assert_file_contains "$active_status" "activated"
