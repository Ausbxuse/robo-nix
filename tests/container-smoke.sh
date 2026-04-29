#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

image="${ROBO_NIX_CONTAINER_IMAGE:-ubuntu:24.04}"
runtime="${ROBO_NIX_CONTAINER_RUNTIME:-}"

if [ -z "$runtime" ]; then
	if command -v docker >/dev/null 2>&1; then
		runtime="docker"
	elif command -v podman >/dev/null 2>&1; then
		runtime="podman"
	else
		echo "docker or podman is required for container smoke tests" >&2
		exit 1
	fi
fi

case "$runtime" in
docker | podman) ;;
*)
	echo "unsupported container runtime: $runtime" >&2
	exit 2
	;;
esac

# shellcheck disable=SC2016
"$runtime" run --rm --privileged \
	-v "${repo_root}:/workspace/robo-nix:ro" \
	-e NIX_CONFIG="experimental-features = nix-command flakes
accept-flake-config = true" \
	"$image" \
	bash -lc '
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends bash ca-certificates curl git xz-utils

mkdir -p /nix
if ! getent group nixbld >/dev/null; then
	groupadd -r nixbld
fi
for index in $(seq 1 10); do
	user="nixbld$index"
	if ! id "$user" >/dev/null 2>&1; then
		useradd -r -g nixbld -G nixbld -M -N -s /usr/sbin/nologin "$user"
	fi
done

curl -L https://nixos.org/nix/install | sh -s -- --no-daemon
if [ -e /root/.nix-profile/etc/profile.d/nix.sh ]; then
	. /root/.nix-profile/etc/profile.d/nix.sh
fi
export PATH="/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"
git config --global --add safe.directory /workspace/robo-nix
nix --version

project=/workspace/container-project
mkdir -p "$project"
cat >"$project/pyproject.toml" <<EOF
[project]
name = "container-project"
requires-python = "==3.11.*"
dependencies = []
EOF

nix run /workspace/robo-nix#robo -- init "$project" --robo-nix-url path:/workspace/robo-nix
cd "$project"
nix run /workspace/robo-nix#robo -- check
nix run /workspace/robo-nix#robo -- status

cat >/tmp/fake-shell <<EOF
#!/usr/bin/env bash
set -euo pipefail
test "\${ROBO_NIX_ACTIVE:-}" = "1"
test "\${ROBO_NIX_ENV_NAME:-}" = "container-project"
test "\${SHELL:-}" = "/tmp/fake-shell"
robo status
EOF
chmod +x /tmp/fake-shell

ROBO_NIX_SHELL=/tmp/fake-shell nix run /workspace/robo-nix#robo -- activate
'
