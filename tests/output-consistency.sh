#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

if rg -n 'println!\("check:|eprintln!\("check:' crates/robo-cli/src; then
	echo "check output must use themed helpers, not raw check: print calls" >&2
	exit 1
fi

if rg -n 'println!\("contract:|eprintln!\("contract:' crates/robo-cli/src; then
	echo "contract output must use themed helpers, not raw contract: print calls" >&2
	exit 1
fi

if rg -n '"diagnosis:"|"confidence:"|"agent handoff:"|"agent prompt"|"handoff commands"|"agent_handoff"|"no known runtime failure matched"' crates/robo-cli/src/diagnose.rs; then
	echo "diagnose human output must use compiler-style diagnostic sections" >&2
	exit 1
fi

if rg -n 'detected runtime changes|shell: detected runtime changes' crates/robo-cli/src/command/project crates/robo-cli/src/command/project/shell_env.rs; then
	echo "shell refresh output must not use shell-prefixed runtime-change messages" >&2
	exit 1
fi

if command -v script >/dev/null 2>&1; then
	robo_bin="$(nix build --no-link --print-out-paths "${repo_flake_url}#robo")/bin/robo"
	help_output="$(script -q -e -c "env -u NO_COLOR '$robo_bin' help" /dev/null)"
	if ! grep "$(printf '\033')" <<<"$help_output" >/dev/null; then
		echo "custom help sections must keep terminal color styling" >&2
		exit 1
	fi
	if ! grep -F "leave the active runtime shell" <<<"$help_output" >/dev/null; then
		echo "help output must explain runtime shell exit" >&2
		exit 1
	fi
	if grep -E "robo hook|robo deactivate|^notes$" <<<"$help_output" >/dev/null; then
		echo "help output must not mention removed hook/deactivate surfaces or notes section" >&2
		exit 1
	fi
fi
