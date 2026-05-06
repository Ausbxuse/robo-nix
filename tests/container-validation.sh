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
		echo "docker or podman is required for container validation checks" >&2
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
	-e ROBO_NIX_CONTAINER_RUN_SHELL="${ROBO_NIX_CONTAINER_RUN_SHELL:-0}" \
	-e NIX_CONFIG="experimental-features = nix-command flakes
accept-flake-config = true
download-attempts = 10
http-connections = 1
extra-substituters = https://nixpkgs-python.cachix.org https://ros.cachix.org
extra-trusted-public-keys = nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU= ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo=" \
	"$image" \
	bash -lc '
set -euo pipefail

install_prereqs() {
	if command -v apt-get >/dev/null 2>&1; then
		export DEBIAN_FRONTEND=noninteractive
		apt-get update
		apt-get install -y --no-install-recommends bash ca-certificates curl git xz-utils
	elif command -v dnf >/dev/null 2>&1; then
		dnf install -y bash ca-certificates curl git shadow-utils xz
	elif command -v microdnf >/dev/null 2>&1; then
		microdnf install -y bash ca-certificates curl git shadow-utils xz
	elif command -v pacman >/dev/null 2>&1; then
		pacman -Sy --noconfirm bash ca-certificates curl git shadow xz
	elif command -v zypper >/dev/null 2>&1; then
		zypper --non-interactive refresh
		zypper --non-interactive install bash ca-certificates curl git shadow xz
	elif command -v nix >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
		return
	else
		echo "unsupported container package manager and no usable Nix environment found" >&2
		exit 1
	fi
}

install_prereqs

if [ -e /root/.nix-profile/etc/profile.d/nix.sh ]; then
	. /root/.nix-profile/etc/profile.d/nix.sh
fi
export PATH="/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"

if ! command -v nix >/dev/null 2>&1; then
	mkdir -p /nix
	if ! getent group nixbld >/dev/null; then
		groupadd -r nixbld
	fi
	nologin_shell=/usr/sbin/nologin
	if [ ! -x "$nologin_shell" ]; then
		nologin_shell=/sbin/nologin
	fi
	if [ ! -x "$nologin_shell" ]; then
		nologin_shell=/bin/false
	fi
	for index in $(seq 1 10); do
		user="nixbld$index"
		if ! id "$user" >/dev/null 2>&1; then
			useradd -r -g nixbld -G nixbld -M -N -s "$nologin_shell" "$user"
		fi
	done

	curl -L https://nixos.org/nix/install | sh -s -- --no-daemon
	if [ -e /root/.nix-profile/etc/profile.d/nix.sh ]; then
		. /root/.nix-profile/etc/profile.d/nix.sh
	fi
	export PATH="/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"
fi

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

if [ "${ROBO_NIX_CONTAINER_RUN_SHELL:-0}" = "1" ]; then
	cat >/tmp/fake-shell <<EOF
#!/usr/bin/env bash
set -euo pipefail
test "\${ROBO_NIX_ACTIVE:-}" = "1"
test "\${ROBO_NIX_ENV_NAME:-}" = "container-project"
test "\${SHELL:-}" = "/tmp/fake-shell"
robo status
EOF
	chmod +x /tmp/fake-shell

	ROBO_NIX_SHELL=/tmp/fake-shell nix run /workspace/robo-nix#robo -- shell
fi
'
