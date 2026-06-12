# Robo Update

## Context

Downstream projects pin `robo-nix` through their generated `flake.lock`.
Updating to a newly pushed robo-nix version currently requires users to know the
Nix flake command:

```bash
nix flake update robo-nix
```

That is reasonable Nix, but it is not a good robo-nix workflow for users who
only need to update the runtime tooling itself.

## Review Ledger

Potential conflict: `robo-nix` should not grow into a general environment or
dependency manager.

Resolution: `robo update` updates the workspace `robo-nix` flake input and
reinstalls the `robo` CLI binary from that updated input. It refuses non-robo
flakes, does not touch `robo.nix`, does not update Python dependencies, and
does not update arbitrary Nix inputs.

## Change

- Add `robo update`.
- Run `nix flake update robo-nix` in the workspace root.
- Reinstall the `robo` CLI binary from the updated locked `robo-nix` input.
- Clear `.robo-nix/profiles/` after a successful lock update so runtime cache
  state is rebuilt from the updated lock.
- In active runtime shells, request prompt-time refresh for the active profile.
- Document the command in user and developer docs.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix build .#checks.x86_64-linux.default --no-link`
