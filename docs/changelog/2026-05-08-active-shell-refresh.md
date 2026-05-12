# 2026-05-08 - Active Shell Refresh

## Goal

Restore the old CLI's useful active-shell behavior: a running `robo shell`
should notice runtime input changes and refresh its environment without asking
the user to exit and re-enter.

## Scope

- Add runtime input fingerprints for `flake.nix`, `flake.lock`,
  `.python-version`, `pyproject.toml`, `uv.lock`, and `robo.nix`.
- Carry the active fingerprint state into interactive shells.
- Add a hidden `robo __shell-refresh <shell>` command used by shell startup
  hooks.
- Update bash, zsh, and fish startup templates to refresh at prompt time.
- Keep refresh limited to exporting a re-evaluated shell environment. It must
  not rewrite `robo.nix`.

## Non-Goals

- No shell environment cache in this change.
- No comment-normalized `robo.nix` fingerprinting yet.
- No migration of the current process into a new Nix shell.
- No automatic `uv sync`.

## Review Notes

Pending concerns:

- Comment-only edits to `robo.nix` currently trigger a refresh because this
  minimal branch fingerprints file bytes. That is safe but less refined than
  the old normalized-manifest path.
- Removed shell variables may persist if they are inherited by the nested
  refresh evaluation. This matches the current export-only shape and should be
  revisited only if a real workflow needs unsetting.

## Verification

Run for this change:

- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- fake-project smoke where a shell edits `pyproject.toml`, runs
  `robo __shell-refresh bash`, and observes a changed runtime key
