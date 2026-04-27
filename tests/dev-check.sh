#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

cd "$repo_root"

cargo check --all-targets --all-features
nix eval .#apps.x86_64-linux.robo.program --raw >/dev/null
nix eval .#apps.x86_64-linux.isaac-ros2-learning.type --raw >/dev/null
bash tests/regression-api.sh --fast
bash tests/robo-init-validation.sh
bash tests/vendor-validation.sh
bash tests/contract-validation.sh
bash tests/output-consistency.sh
