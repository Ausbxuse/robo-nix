# Iteration 010 - Original CLI UX Match

## Goal

Match the original Rust CLI presentation more closely while keeping the rewrite
branch command surface.

## Scope

- Replace the minimal manual spinner with the original `indicatif` braille
  spinner format.
- Use original-style lowercase sections for generated files and runtime
  inference.
- Preflight `nix develop` with a spinner, then run user commands without an
  active spinner.
- Launch the user's default interactive shell for `robo shell` instead of
  relying on Nix's default Bash.
- Add the original `[robo]` prompt prefix for Bash, Zsh, and Fish through
  checked-in startup templates.

## Non-Goals

- No shell environment cache.
- No `robo init`.
- No `robo check`.
- No `robo diagnose`.
- No hidden uv sync.

## Review Notes

Pending concerns:

- `robo shell` now evaluates `nix develop --command true` before launching the
  interactive shell. This intentionally gives the spinner a bounded phase, but
  it means shell entry may evaluate Nix twice.
- Prompt prefix scripts are intentionally static templates. They should stay in
  `templates/shell/`, not inline Rust strings.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `npm --prefix docs run build`
- `nix build .#robo --accept-flake-config`
- `nix flake check --accept-flake-config`
- fake-`nix` smoke tests for captured output
- a TTY smoke test for spinner and shell launch command shape
