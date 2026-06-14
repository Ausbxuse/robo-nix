# Active Profile Switch

## Context

Inside an active runtime shell, a project user tried to move from the current
profile into the `training` profile:

```bash
robo shell -p training
```

`robo` rejected the command as a nested shell. Running `robo refresh` afterward
kept refreshing the active profile, so console scripts installed into the
training virtualenv were still not on `PATH`.

## Change

- Keep refusing plain nested `robo shell`.
- Treat `robo shell --profile <name>` inside an active shell as an in-place
  active shell profile switch request.
- Let the prompt refresh hook consume that request before choosing which
  profile to evaluate.
- Force one refresh for the requested profile even when the previous active
  runtime input key matches.

## Verification

- [x] `cargo fmt`
- [x] `cargo test`
- [x] `cargo build`
- [x] active-shell smoke test writes `active-profile-switch-request-v1`
- [x] `nix-instantiate --parse flake.nix`
