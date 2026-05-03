#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tests/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

image="${ROBO_NIX_UBUNTU_IMAGE:-ubuntu:24.04}"
container="${ROBO_NIX_UBUNTU_CONTAINER:-robo-nix-ubuntu-smoke}"
keep=0
run_gui=0
projects=()

usage() {
	cat >&2 <<'EOF'
usage: tests/ubuntu-downstream-smoke.sh [--keep] [--gui] PROJECT...

Run downstream project smoke tests inside an Ubuntu Podman container.

Environment:
  ROBO_NIX_UBUNTU_IMAGE       Ubuntu image to use, default ubuntu:24.04
  ROBO_NIX_UBUNTU_CONTAINER   Container name, default robo-nix-ubuntu-smoke

Examples:
  tests/ubuntu-downstream-smoke.sh ~/src/dev/dexmate ~/src/dev/py-learn
  tests/ubuntu-downstream-smoke.sh --gui ~/src/dev/dexmate
EOF
}

while [ "$#" -gt 0 ]; do
	case "$1" in
	--keep)
		keep=1
		;;
	--gui)
		run_gui=1
		;;
	-h | --help)
		usage
		exit 0
		;;
	--)
		shift
		while [ "$#" -gt 0 ]; do
			projects+=("$1")
			shift
		done
		break
		;;
	-*)
		echo "unknown option: $1" >&2
		usage
		exit 2
		;;
	*)
		projects+=("$1")
		;;
	esac
	shift
done

if [ "${#projects[@]}" -eq 0 ]; then
	usage
	exit 2
fi

if ! command -v podman >/dev/null 2>&1; then
	echo "podman is required for Ubuntu downstream smoke tests" >&2
	exit 1
fi

mount_args=(-v "${repo_root}:/workspace/robo-nix:ro")
container_project_args=()
index=0
for project in "${projects[@]}"; do
	host_project="$(realpath "$project")"
	if [ ! -d "$host_project" ]; then
		echo "project does not exist: $project" >&2
		exit 1
	fi
	name="$(basename "$host_project")"
	input_project="/workspace/inputs/${index}-${name}"
	smoke_project="/workspace/projects/${index}-${name}"
	mount_args+=(-v "${host_project}:${input_project}:ro")
	container_project_args+=("$input_project" "$smoke_project")
	index=$((index + 1))
done

cleanup() {
	if [ "$keep" -eq 0 ]; then
		podman rm -f "$container" >/dev/null 2>&1 || true
	fi
}
trap cleanup EXIT

if podman container exists "$container"; then
	podman rm -f "$container" >/dev/null
fi

podman run -d \
	--name "$container" \
	--privileged \
	-v "${container}-nix:/nix" \
	-v "${container}-root:/root" \
	"${mount_args[@]}" \
	"$image" \
	sleep infinity >/dev/null

podman exec "$container" bash -lc '
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends bash ca-certificates curl git sudo xz-utils
if [ -e /root/.nix-profile/etc/profile.d/nix.sh ]; then
	. /root/.nix-profile/etc/profile.d/nix.sh
fi
export PATH="/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"
if ! command -v nix >/dev/null 2>&1; then
	if [ ! -e /root/.nix-profile/bin/nix ] && [ ! -e /nix/var/nix/profiles/default/bin/nix ]; then
		rm -rf /nix/*
	fi
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
fi
nix --version
'

podman exec \
	-e NIX_CONFIG="experimental-features = nix-command flakes" \
	-e ROBO_NIX_UBUNTU_GUI="$run_gui" \
	"$container" \
	bash -lc '
set -euo pipefail
if [ -e /root/.nix-profile/etc/profile.d/nix.sh ]; then
	. /root/.nix-profile/etc/profile.d/nix.sh
fi
export PATH="/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"
git config --global --add safe.directory /workspace/robo-nix

while [ "$#" -gt 0 ]; do
	input_project="$1"
	project="$2"
	shift 2

	echo "ubuntu-smoke: project=$project"
	rm -rf "$project"
	mkdir -p "$(dirname "$project")"
	cp -a "$input_project" "$project"
	cd "$project"

	nix run /workspace/robo-nix#robo -- init . --robo-nix-url path:/workspace/robo-nix
	nix run /workspace/robo-nix#robo -- check
	nix run /workspace/robo-nix#robo -- shell -c "uv sync"

	if [ "${ROBO_NIX_UBUNTU_GUI:-0}" = "1" ]; then
		bash /workspace/robo-nix/tests/gui-runtime-smoke.sh "$project"
	fi
done
' bash "${container_project_args[@]}"

if [ "$keep" -eq 1 ]; then
	echo "kept Ubuntu smoke container: $container"
	echo "enter with: podman exec -it $container bash"
fi
