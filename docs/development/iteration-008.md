# Iteration 008 - Docs And CLI UX

## Goal

Bring back the old repo's VitePress docs presentation and CLI polish without
bringing back old product surfaces.

## Scope

- Add VitePress docs tooling under `docs/`.
- Add current-branch docs for `robo shell`, `robo run`, runtime ownership, and
  generated project files.
- Add a CLI-styled animated terminal panel to the docs homepage.
- Add Rust CLI label styling, color control, and a lightweight spinner for
  bounded `robo run` calls that go through `nix develop`.

## Non-Goals

- No `robo init`.
- No `robo check`.
- No `robo diagnose`.
- No root Node package files.
- No broad CLI framework or extra command surface.

## Review Notes

Pending concerns:

- Actual CLI styling must remain quiet and stable for captured output.
- Docs copied from the old repo must be rewritten when they reference removed
  commands.

## Verification

Run for this iteration:

- `cargo test`
- `cargo check`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix flake check --accept-flake-config`
- `npm --prefix docs run build`
- Smoke `robo shell` with a fake `nix` to inspect styled captured output.
