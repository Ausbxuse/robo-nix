#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

tmpdir="$(mktemp_dir)"
trap 'cleanup_dir "$tmpdir"' EXIT

robo_nix_url="git+file://${repo_root}"
robo="$(nix build --no-warn-dirty "${robo_nix_url}#robo" --no-link --print-out-paths)/bin/robo"

project="$tmpdir/project"
mkdir -p "$project"
cat >"$project/flake.nix" <<EOF
{
  inputs.robo-nix.url = "${robo_nix_url}";
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
assert_file_contains "$outside_status" "checked shell-shell-project"
assert_file_contains "$outside_status" "uv.lock missing"
assert_file_contains "$outside_status" "Python environment missing"

init_build_project="$tmpdir/init-build-project"
init_build_output="$tmpdir/init-build.txt"
"$robo" init "$init_build_project" --build --robo-nix-url "$robo_nix_url" >"$init_build_output"
test -s "$init_build_project/.robo-nix/shell-env"
test -s "$init_build_project/.robo-nix/shell-env.key"
assert_file_contains "$init_build_output" "robo runtime is built for this project."

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
test "${ROBO_NIX_PROMPT_PREFIX:-}" = "[robo]"
command -v robo >/dev/null
robo status >/dev/null
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
	assert_file_contains "$output" "prompt-prefix=[robo]"
done

build_shell="$(make_fake_shell build-shell)"
build_shell_output="$tmpdir/build-shell.txt"
(
	cd "$project"
	"$robo" build >"$build_shell_output"
	test -s .robo-nix/shell-env
	test -s .robo-nix/shell-env.key
	ROBO_NIX_SHELL="$build_shell" EXPECTED_SHELL="$build_shell" \
		"$robo" shell >>"$build_shell_output"
)
assert_file_contains "$build_shell_output" "robo runtime is built for this project."
assert_file_contains "$build_shell_output" "active=1"
assert_file_contains "$build_shell_output" "env=shell-shell-project"

active_status="$tmpdir/active-status.txt"
(
	cd "$project"
	ROBO_NIX_ACTIVE=1 \
		ROBO_NIX_ENV_NAME=shell-shell-project \
		ROBO_NIX_PYTHON_VERSION=3.11 \
		WORKSPACE_ROOT="$project" \
		ROBO_NIX_PROMPT_PREFIX="[robo]" \
		"$robo" status >"$active_status"
)
assert_file_contains "$active_status" "checked shell-shell-project"
assert_file_contains "$active_status" "uv.lock missing"
assert_file_contains "$active_status" "Python environment missing"

bash_prompt_status="$tmpdir/bash-prompt-status.txt"
(
	cd "$project"
	{
		cat <<'EOF'
case "$PS1" in
  *"90m"*'['*"37m"*ro*"36m"*bo*"90m"*']'*) ;;
  *) echo "missing prompt prefix: $PS1" >&2; exit 1 ;;
esac
case "$PS1" in
  *"◆ shell-shell-project"*) echo "env prompt prefix still present: $PS1" >&2; exit 1 ;;
esac
case "$PS1" in
  *"<shell-shell-project>"*) echo "old prompt prefix still present: $PS1" >&2; exit 1 ;;
esac
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
robo status
exit
EOF
	} | ROBO_NIX_SHELL="$(command -v bash)" "$robo" shell >"$bash_prompt_status"
)
assert_file_contains "$bash_prompt_status" "checked shell-shell-project"

if command -v zsh >/dev/null 2>&1; then
	zsh_prompt_status="$tmpdir/zsh-prompt-status.txt"
	(
		cd "$project"
		{
			cat <<'EOF'
case "$PS1" in
  *"%F{8}["*"%F{white}ro"*"%F{cyan}bo"*"%F{8}]"*) ;;
  *) echo "missing zsh prompt prefix: $PS1" >&2; exit 1 ;;
esac
case "$PS1" in
  *"◆ shell-shell-project"*) echo "env zsh prompt prefix still present: $PS1" >&2; exit 1 ;;
esac
case "$PS1" in
  *"<shell-shell-project>"*) echo "old zsh prompt prefix still present: $PS1" >&2; exit 1 ;;
esac
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
robo status
exit
EOF
		} | ROBO_NIX_SHELL="$(command -v zsh)" "$robo" shell >"$zsh_prompt_status"
	)
	assert_file_contains "$zsh_prompt_status" "checked shell-shell-project"
fi

if command -v fish >/dev/null 2>&1; then
	fish_prompt_status="$tmpdir/fish-prompt-status.txt"
	(
		cd "$project"
		{
			cat <<'EOF'
string match -q "*ro*bo*" (fish_prompt)
or begin; echo "missing fish prompt prefix" >&2; exit 1; end
test "$ROBO_NIX_ENV_NAME" = "shell-shell-project"
robo status
exit
EOF
		} | ROBO_NIX_SHELL="$(command -v fish)" "$robo" shell >"$fish_prompt_status"
	)
	assert_file_contains "$fish_prompt_status" "checked shell-shell-project"
fi
