#!/usr/bin/env sh

set -eu

robo_nix_flake="${ROBO_NIX_FLAKE:-github:ausbxuse/robo-nix/master}"
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

ensure_user_nix_config() {
	nix_config_dir="$HOME/.config/nix"
	nix_config_file="$nix_config_dir/nix.conf"

	if [ ! -d "$nix_config_dir" ]; then
		info "creating user Nix config directory: $nix_config_dir"
		mkdir -p "$nix_config_dir"
	fi

	if [ ! -f "$nix_config_file" ]; then
		info "creating user Nix config: $nix_config_file"
		printf '%s\n' "experimental-features = nix-command flakes" >"$nix_config_file"
		return
	fi

	if ! grep -Eq '^[[:space:]]*(extra-)?experimental-features[[:space:]]*=.*(^|[[:space:]])nix-command([[:space:]]|$)' "$nix_config_file" \
		|| ! grep -Eq '^[[:space:]]*(extra-)?experimental-features[[:space:]]*=.*(^|[[:space:]])flakes([[:space:]]|$)' "$nix_config_file"; then
		info "enabling Nix flakes in user config: $nix_config_file"
		{
			printf '\n'
			printf '%s\n' "# Added by robo-nix installer."
			printf '%s\n' "extra-experimental-features = nix-command flakes"
		} >>"$nix_config_file"
	fi
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
	need_command mktemp
	info "installing Nix with the Determinate installer"
	tmp_installer="$(mktemp "${TMPDIR:-/tmp}/robo-nix-installer.XXXXXX")"
	trap 'rm -f "$tmp_installer"' EXIT HUP INT TERM
	curl --proto '=https' --tlsv1.2 -fsSL "$nix_installer_url" -o "$tmp_installer"
	sh "$tmp_installer" install --determinate --no-confirm
	rm -f "$tmp_installer"
	trap - EXIT HUP INT TERM
	load_nix_profile
	command -v nix >/dev/null 2>&1 || fail "Nix installed, but nix is not available in this shell; open a new terminal and rerun this installer"
}

install_robo() {
	info "installing robo from $robo_nix_flake"
	nix --extra-experimental-features nix-command \
		--extra-experimental-features flakes \
		profile remove robo >/dev/null 2>&1 || true
	nix --extra-experimental-features nix-command \
		--extra-experimental-features flakes \
		--accept-flake-config \
		profile install "$robo_nix_flake#robo"

	if ! command -v robo >/dev/null 2>&1; then
		load_nix_profile
	fi
	command -v robo >/dev/null 2>&1 || fail "robo installed, but it is not on PATH; open a new terminal and run robo shell from your project"
}

load_nix_profile
if command -v nix >/dev/null 2>&1; then
	info "using existing Nix: $(nix --version)"
else
	install_nix
fi

ensure_user_nix_config
install_robo
info "done"
info "try: cd your-project && uv python pin 3.11 && robo shell"
