# Command UI Consistency

## Context

`robo refresh` and `robo update` used plain status lines for successful
actions, while other short commands use lowercase sections and row-style
success markers.

## Review Ledger

No conflict blocks this change. The command surface stays narrow; this only
aligns output presentation for existing commands.

## Change

- Print `robo refresh` success output with a `refresh` section and `✓` rows.
- Print `robo update` success output with an `update` section and `✓` rows.
- Keep a spinner/status line for the potentially slow `nix flake update
  robo-nix` step.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix build .#checks.x86_64-linux.default --no-link`
