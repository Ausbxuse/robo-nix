# 2026-05-14 - Run Command Separator

## Concern

`robo run` wraps a child command, but its documented surface did not say whether
it accepted the standard `--` separator before the child argv. That leaves users
guessing when a child command name starts with `-`, and it makes robo feel less
like normal Unix wrapper tools.

No conflicting review-ledger entries were found. Existing product rules keep
`robo run` as the command execution surface and keep `robo shell` focused on
launching an interactive shell.

## Convention Check

- POSIX Utility Syntax Guideline 10 treats the first `--` delimiter as the end
  of wrapper option processing, with following arguments as operands.
- GNU `getopt` also treats `--` as the forced end of option scanning.
- Cargo documents `cargo run [options] [-- args]` and passes arguments after the
  separator to the binary.

## Decision

- Support `robo run [--] <command> [args...]`.
- Strip exactly one leading `--` immediately after `run`.
- Require a command after the separator.
- Preserve every `--` that appears after the command name for the child command.
- Do not add `robo shell -- <command>`; `robo run` remains the execution
  surface for commands.

## Verification

Run for this change:

- `cargo test`
- `nix-instantiate --parse flake.nix`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `git diff --check`
