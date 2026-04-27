#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

if rg -n 'println!\("doctor:|eprintln!\("doctor:' crates/robo/src; then
	echo "doctor output must use themed helpers, not raw doctor: print calls" >&2
	exit 1
fi

if rg -n 'println!\("vendor:|eprintln!\("vendor:' crates/robo/src/vendor.rs; then
	echo "vendor output must use themed helpers, not raw vendor: print calls" >&2
	exit 1
fi
