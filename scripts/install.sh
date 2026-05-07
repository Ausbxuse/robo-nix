#!/usr/bin/env sh

set -eu

robo_nix_flake="${ROBO_NIX_FLAKE:-github:ausbxuse/robo-nix/develop}"
nix_installer_url="${ROBO_NIX_NIX_INSTALLER_URL:-https://install.determinate.systems/nix}"

info() {
	printf 'robo installer: %s\n' "$*"
}

fail() {
	printf 'robo installer: error: %s\n' "$*" >&2
	exit 1
}

need_command() {
	command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

load_nix_profile() {
	if [ -r /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]; then
		# shellcheck disable=SC1091
		. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
	fi
	if [ -r "$HOME/.nix-profile/etc/profile.d/nix.sh" ]; then
		# shellcheck disable=SC1091
		. "$HOME/.nix-profile/etc/profile.d/nix.sh"
	fi
	PATH="/nix/var/nix/profiles/default/bin:$HOME/.nix-profile/bin:$PATH"
	export PATH
}

install_nix() {
	need_command curl
	info "installing Nix with the Determinate installer"
	curl --proto '=https' --tlsv1.2 -fsSL "$nix_installer_url" | sh -s -- install --determinate --no-confirm
	load_nix_profile
	command -v nix >/dev/null 2>&1 || fail "Nix installed, but nix is not available in this shell; open a new terminal and rerun this installer"
}

install_robo() {
	info "installing robo from $robo_nix_flake"
	nix profile remove robo >/dev/null 2>&1 || true
	nix --extra-experimental-features nix-command \
		--extra-experimental-features flakes \
		--accept-flake-config \
		profile install "$robo_nix_flake#robo"
}

load_nix_profile
if command -v nix >/dev/null 2>&1; then
	info "using existing Nix: $(nix --version)"
else
	install_nix
fi

install_robo
info "done"
info "try: robo up --shell"
