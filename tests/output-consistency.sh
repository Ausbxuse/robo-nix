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
