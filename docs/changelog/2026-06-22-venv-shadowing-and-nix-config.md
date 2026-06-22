# Venv Shadowing and Nix Config Setup

Problem: a project can contain a stale `.venv` while `robo shell` prepares a
robo-owned uv environment. If the user's shell startup auto-activates `.venv`
after robo launches the interactive shell, `python` can resolve to the wrong
environment and imports fail even though `uv sync` installed the package in the
robo runtime environment.

Change:
- Print a one-time shell warning when `python` resolves outside
  `UV_PROJECT_ENVIRONMENT/bin`.
- Include concrete cleanup guidance: deactivate active venvs, stop
  auto-activating `.venv`, then run `robo shell` and `uv sync`.
- Make the installer create `~/.config/nix/nix.conf` when absent and enable
  `experimental-features = nix-command flakes` when the user config has no
  explicit experimental-features setting.

Verification:
- `sh -n scripts/install.sh`
- `cargo fmt --check`
- `cargo test`
