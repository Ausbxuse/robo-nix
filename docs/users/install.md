# Install

The installer installs the `robo` CLI through Nix profiles. If Nix is missing,
it uses the Determinate Nix installer first.

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/rewrite/scripts/install.sh | sh
```

Then start from a Python project:

```bash
uv python pin 3.11
robo shell
uv sync
```

## Local Checkout

When testing this branch from a local checkout, point the installer at that
checkout:

```bash
ROBO_NIX_FLAKE="path:$PWD" ./scripts/install.sh
```

Manual profile install:

```bash
nix profile remove robo || true
nix profile add .#robo
```

`nix profile add .#` also installs the CLI, but Nix names that profile entry
after the checkout directory instead of the package alias. Use `.#robo` when you
want `nix profile remove robo` to keep working for future reinstalls.

## Environment Overrides

`ROBO_NIX_FLAKE` changes the flake installed by the script.

`ROBO_NIX_NIX_INSTALLER_URL` changes the Nix installer URL used when Nix is not
already installed.

The installer does not create project files directly. Project bootstrap still
happens through `robo shell` inside the target project.
