# Shell Refresh Prompt And Interrupt Handling

## Context

Active `robo shell` refresh can run at the next prompt after runtime inputs
change. Two regressions were reported from that path:

- After changing directories, an automatic refresh could leave the prompt
  displaying an older directory.
- Pressing Ctrl-C during refresh did not stop the prompt hook cleanly.

## Review Ledger

No conflict blocks this fix. Prompt hooks should keep shell-local state such as
the current directory owned by the interactive shell, while refresh should only
export runtime environment changes. Interrupts should leave the active shell
usable and should not continue the prompt hook as if refresh succeeded.

## Change

- Exclude `PWD` and `OLDPWD` from captured and cached runtime environments.
- Make bash, zsh, and fish prompt hooks stop prompt-prefix work when refresh
  exits non-zero, including Ctrl-C.
- Track `ROBO_NIX_PROFILE_SELECTOR` as a robo-managed active-shell variable.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix build .#checks.x86_64-linux.default --no-link`
