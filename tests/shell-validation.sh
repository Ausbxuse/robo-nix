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
assert_file_contains "$outside_status" "shell-shell-project  ok  python=3.11"
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
test "${ROBO_NIX_ENV_NAME:-}" = "${EXPECTED_ENV:-shell-shell-project}"
test "${SHELL:-}" = "${EXPECTED_SHELL:?}"
test "${ROBO_NIX_PROMPT_PREFIX:-}" = "[robo]"
command -v robo >/dev/null
robo status >/dev/null
robo shell
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
	assert_file_contains "$output" "run exit to leave this runtime shell"
done

stale_shell="$(make_fake_shell stale-shell)"
stale_output="$tmpdir/stale-shell.txt"
cat >"$stale_shell" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

old_key="${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
printf '\n# edited after shell launch\n' >> pyproject.toml
eval "$(robo __shell-refresh bash)"
test -n "${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
test "${ROBO_NIX_RUNTIME_INPUT_KEY:-}" != "$old_key"
robo shell
EOF
chmod +x "$stale_shell"
(
	cd "$project"
	ROBO_NIX_SHELL="$stale_shell" EXPECTED_SHELL="$stale_shell" \
		"$robo" shell >"$stale_output" 2>&1
)
assert_file_contains "$stale_output" "runtime changed in $project"
assert_file_contains "$stale_output" "changed $project/pyproject.toml"
if grep -F "shell: detected runtime changes" "$stale_output" >/dev/null; then
	echo "shell refresh should not print shell-prefixed status lines" >&2
	exit 1
fi
assert_file_contains "$stale_output" "run exit to leave this runtime shell"

comment_shell="$tmpdir/comment-shell"
comment_output="$tmpdir/comment-shell.txt"
cat >"$comment_shell" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

old_key="${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
printf '\n# comment-only edit after shell launch\n' >> robo.nix
eval "$(robo __shell-refresh bash)"
test -n "${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
test "${ROBO_NIX_RUNTIME_INPUT_KEY:-}" = "$old_key"
robo shell
EOF
chmod +x "$comment_shell"
(
	cd "$project"
	ROBO_NIX_SHELL="$comment_shell" EXPECTED_SHELL="$comment_shell" \
		"$robo" shell >"$comment_output" 2>&1
)
if grep -F "runtime changed in $project" "$comment_output" >/dev/null; then
	echo "comment-only robo.nix edits should not refresh the runtime" >&2
	exit 1
fi
assert_file_contains "$comment_output" "run exit to leave this runtime shell"

shell_init_shell="$tmpdir/shell-init-shell"
shell_init_output="$tmpdir/shell-init-shell.txt"
cat >"$shell_init_shell" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

old_key="${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
tmp="$(mktemp)"
awk '
  /^}$/ && !done {
    print "  shellInit = \"export ROBO_SHELL_REFRESH_TEST=1\";"
    done = 1
  }
  { print }
' robo.nix >"$tmp"
mv "$tmp" robo.nix
eval "$(robo __shell-refresh bash)"
test -n "${ROBO_NIX_RUNTIME_INPUT_KEY:-}"
test "${ROBO_NIX_RUNTIME_INPUT_KEY:-}" != "$old_key"
test "${ROBO_SHELL_REFRESH_TEST:-}" = "1"
robo shell
EOF
chmod +x "$shell_init_shell"
(
	cd "$project"
	ROBO_NIX_SHELL="$shell_init_shell" EXPECTED_SHELL="$shell_init_shell" \
		"$robo" shell >"$shell_init_output" 2>&1
)
assert_file_contains "$shell_init_output" "runtime changed in $project"
assert_file_contains "$shell_init_output" "changed $project/robo.nix"
if grep -F "shell: detected runtime changes" "$shell_init_output" >/dev/null; then
	echo "shell refresh should not print shell-prefixed status lines" >&2
	exit 1
fi
assert_file_contains "$shell_init_output" "run exit to leave this runtime shell"

prompt_init_project="$tmpdir/prompt-init-project"
prompt_init_shell="$(make_fake_shell prompt-init-shell)"
prompt_init_output="$tmpdir/prompt-init-shell.txt"
mkdir -p "$prompt_init_project"
if command -v script >/dev/null 2>&1; then
	printf '\n' | script -q -e -c \
		"cd '$prompt_init_project' && env ROBO_NIX_SHELL='$prompt_init_shell' EXPECTED_SHELL='$prompt_init_shell' EXPECTED_ENV=project '$robo' shell" \
		/dev/null >"$prompt_init_output"
	test -s "$prompt_init_project/flake.nix"
	test -s "$prompt_init_project/robo.nix"
	test -s "$prompt_init_project/pyproject.toml"
	assert_file_contains "$prompt_init_output" "No robo runtime files were found"
	assert_file_contains "$prompt_init_output" "active=1"
	assert_file_contains "$prompt_init_output" "env=project"
fi

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
assert_file_contains "$active_status" "shell-shell-project  ok  python=3.11"
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
assert_file_contains "$bash_prompt_status" "shell-shell-project  ok  python=3.11"

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
ansi_robo_prefix=$'\e[90m[\e[39m\e[37mro\e[39m\e[36mbo\e[39m\e[90m]\e[39m'
robo_prompt_prefix='%F{8}[%f%F{white}ro%f%F{cyan}bo%f%F{8}]%f'
PROMPT="${ansi_robo_prefix}${PROMPT}"
PS1="$PROMPT"
precmd
case "$PS1" in
  "$robo_prompt_prefix$ansi_robo_prefix"*) echo "rendered zsh prompt prefix duplicated: $PS1" >&2; exit 1 ;;
esac
precmd
case "$PS1" in
  "$robo_prompt_prefix$robo_prompt_prefix"*) echo "zsh prompt prefix duplicated after redraw: $PS1" >&2; exit 1 ;;
esac
test "${ROBO_NIX_ENV_NAME:-}" = "shell-shell-project"
robo status
exit
EOF
		} | ROBO_NIX_SHELL="$(command -v zsh)" "$robo" shell >"$zsh_prompt_status"
	)
	assert_file_contains "$zsh_prompt_status" "shell-shell-project  ok  python=3.11"

	if command -v script >/dev/null 2>&1; then
		zsh_blank_prompt_output="$tmpdir/zsh-blank-prompts.txt"
		zsh_blank_prompt_visible="$tmpdir/zsh-blank-prompts-visible.txt"
		(
			cd "$project"
			{
				printf '\n\n\nexit\n'
			} | script -q -e -c \
				"env ROBO_NIX_SHELL='$(command -v zsh)' '$robo' shell" \
				/dev/null >"$zsh_blank_prompt_output"
		)
		perl -pe 's/\e\[[0-9;?]*[ -\/]*[@-~]//g; s/\e\][^\a]*(\a|\e\\)//g; s/\r/\n/g' \
			"$zsh_blank_prompt_output" >"$zsh_blank_prompt_visible"
		if grep -F "[robo][robo]" "$zsh_blank_prompt_visible" >/dev/null; then
			printf 'zsh prompt prefix duplicated after blank prompts\n' >&2
			cat "$zsh_blank_prompt_visible" >&2
			exit 1
		fi
	fi
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
	assert_file_contains "$fish_prompt_status" "shell-shell-project  ok  python=3.11"
fi
