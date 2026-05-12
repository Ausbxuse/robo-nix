# 2026-05-08 - Nested Progress Trees

## Goal

Restore the original CLI's nested setup tree for bounded `robo shell` and
`robo run` preparation work.

## Scope

- Replace the one-line preflight spinner with a compact progress tree.
- Keep non-interactive and debug output as plain `phase: detail` lines.
- Capture Nix stderr while preflight runs so the live tree can show useful
  detail rows and evaluated package counts without dumping noisy logs on
  success.
- Keep failed preflight behavior unchanged: clear the tree, print captured
  stdout/stderr, and return the existing error/hint path.
- Add render tests for tree shape, detail cleanup, spinner frames, and duration
  formatting.

## Non-Goals

- No shell environment cache.
- No live tree while the user's interactive shell or command owns the terminal.
- No `robo check`, `robo init`, or `robo diagnose` surface.
- No attempt to parse every Nix progress format.

## Review Notes

Pending concerns:

- The tree currently wraps only the bounded `nix develop --command true`
  preflight. That keeps command output ownership clean, but `robo shell` still
  evaluates Nix twice before entering the interactive shell.
- Nix progress details are intentionally lightweight: count unique
  `package.nix` evaluations and show the last few non-empty, non-control detail
  rows. Broader Nix log classification should stay out of this change unless
  downstream logs prove a reusable need.

## Verification

Run for this change:

- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- fake-`nix` TTY smoke for nested tree output
