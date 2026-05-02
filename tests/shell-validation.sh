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
  envName = "shell-shell-project";
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
name = "shell-shell-project"
requires-python = "==3.11.*"
dependencies = []
EOF

outside_status="$tmpdir/outside-status.txt"
(
	cd "$project"
	"$robo" status >"$outside_status"
)
assert_file_contains "$outside_status" "state=inactive"
assert_file_contains "$outside_status" "robo shell"

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
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
test "${SHELL:-}" = "${EXPECTED_SHELL:?}"
test "${ROBO_NIX_PROMPT_PREFIX:-}" = "<shell-shell-project> "
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
	output="$tmpdir/shell-$shell_name.txt"
	(
		cd "$project"
		ROBO_NIX_SHELL="$fake_shell" EXPECTED_SHELL="$fake_shell" \
			"$robo" shell >"$output"
	)
	assert_file_contains "$output" "active=1"
	assert_file_contains "$output" "env=shell-shell-project"
	assert_file_contains "$output" "shell=$fake_shell"
	assert_file_contains "$output" "prompt-prefix=<shell-shell-project> "
done

active_status="$tmpdir/active-status.txt"
(
	cd "$project"
	ROBO_NIX_ACTIVE=1 \
		ROBO_NIX_ENV_NAME=shell-shell-project \
		ROBO_NIX_PYTHON_VERSION=3.11 \
		WORKSPACE_ROOT="$project" \
		ROBO_NIX_PROMPT_PREFIX="<shell-shell-project> " \
		"$robo" status >"$active_status"
)
assert_file_contains "$active_status" "state=active"
assert_file_contains "$active_status" "uv sync"
assert_file_contains "$active_status" "leave this runtime shell"

hook_output="$tmpdir/hook.txt"
"$robo" hook bash >"$hook_output"
assert_file_contains "$hook_output" "__shell-env"
assert_file_contains "$hook_output" "robo()"

hook_status="$tmpdir/hook-status.txt"
(
	cd "$project"
	# shellcheck disable=SC2016,SC2046
	ROBO_BIN="$robo" bash -lc '
set -euo pipefail
eval $("$ROBO_BIN" hook bash)
PS1="$ "
robo shell >/dev/null
test "${ROBO_NIX_ACTIVE:-}" = "1"
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
case "$PS1" in
  "<shell-shell-project> "*) ;;
  *) echo "missing prompt prefix: $PS1" >&2; exit 1 ;;
esac
robo status
robo deactivate
test -z "${ROBO_NIX_ACTIVE:-}"
test "$PS1" = "$ "
' >"$hook_status"
)
assert_file_contains "$hook_status" "state=active"

if command -v zsh >/dev/null 2>&1; then
	zsh_hook_status="$tmpdir/zsh-hook-status.txt"
	(
		cd "$project"
		ROBO_BIN="$robo" zsh -lc '
set -e
eval $("$ROBO_BIN" hook zsh)
PS1="$ "
robo shell >/dev/null
test "${ROBO_NIX_ACTIVE:-}" = "1"
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
case "$PS1" in
  "<shell-shell-project> "*) ;;
  *) echo "missing zsh prompt prefix: $PS1" >&2; exit 1 ;;
esac
robo status
robo deactivate
test -z "${ROBO_NIX_ACTIVE:-}"
test "$PS1" = "$ "
' >"$zsh_hook_status"
	)
	assert_file_contains "$zsh_hook_status" "state=active"
fi
