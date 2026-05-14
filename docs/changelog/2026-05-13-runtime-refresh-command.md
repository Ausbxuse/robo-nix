# 2026-05-13 - Runtime Refresh Command

## Concern

Users sometimes need a manual reset when the local runtime state under
`.robo-nix/` becomes confusing, too stale to trust, or just needs to be rebuilt
without hand-removing files.

An active runtime shell cannot be mutated directly by a child process after that
child exits. A public refresh command should therefore clear robo-owned state
and request the existing prompt hook to refresh the parent shell at the next
prompt.

## Conflict Check

- This intentionally adds one public command, which conflicts with the previous
  "no new public commands" scope note.
- Keep the exception narrow: no `init`, `check`, `diagnose`, service manager,
  background daemon, or full-screen UI.
- Do not rewrite existing `robo.nix`.
- Do not run `uv sync` or resolve Python dependencies.

The command is acceptable as a bounded runtime-cache reset and active-shell
refresh request, not as a broader environment management surface.

## Scope

- Add `robo refresh`.
- Clear robo-owned local state under `.robo-nix/`.
- When run inside an active `robo shell`, request a prompt-time environment
  refresh using the existing shell hook path.
- Keep outside-shell behavior useful by clearing the cache so the next
  `robo shell` or `robo run` rebuilds it.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `git diff --check`
- [x] temp-project `robo refresh` smoke outside an active shell
- [x] temp-project `robo refresh` smoke with `ROBO_NIX_ACTIVE=1`
