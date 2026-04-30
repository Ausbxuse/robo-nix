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

if command -v script >/dev/null 2>&1; then
	help_output="$(script -q -e -c 'env -u NO_COLOR cargo run -q -p robo-cli -- help' /dev/null)"
	if ! grep "$(printf '\033')" <<<"$help_output" >/dev/null; then
		echo "custom help sections must keep terminal color styling" >&2
		exit 1
	fi
	if ! grep -F "eval \"\$(robo hook)\"" <<<"$help_output" >/dev/null; then
		echo "help output must explain prompt-prefix hook setup" >&2
		exit 1
	fi
fi
