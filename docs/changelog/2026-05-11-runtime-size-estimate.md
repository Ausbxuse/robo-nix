# 2026-05-11 - Runtime Size Estimate

## Concern

Large robot-learning runtime shells can realize substantial Nix store closures.
Users should see an approximate store footprint before `robo shell` or
`robo run` starts realization so a long setup is less surprising.

## Decision

- When no runtime cache is available, evaluate the dev-shell derivation.
- Ask Nix for the derivation requisites including output paths.
- Sum known `nix path-info --size` values and report the approximate runtime
  closure size before `nix develop` realizes the shell.
- Keep the estimate best effort: failure to estimate must not block the runtime
  shell.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
