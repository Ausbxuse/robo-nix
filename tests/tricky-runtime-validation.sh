#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

python_bin="${PYTHON:-python3}"
if [ -x ".venv/bin/python" ]; then
	python_bin=".venv/bin/python"
elif ! command -v "$python_bin" >/dev/null 2>&1; then
	echo "SKIP tricky runtime validation - missing python" >&2
	exit 0
fi

"$python_bin" tests/tricky-runtime-validation.py
