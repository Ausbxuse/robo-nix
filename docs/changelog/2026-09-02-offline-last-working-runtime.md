# Offline Last-Working Runtime Fallback

## Concern

The runtime environment cache is only reused when its input key is current. If
runtime inputs change and `nix develop` cannot evaluate them without network
access, `robo shell` and `robo run` fail even when the previous cached runtime
environment is still locally usable.

The cached environment also only records referenced Nix store paths. It does
not retain them as garbage-collection roots, so a later Nix collection can
remove the local paths needed for offline reuse.

The failure path is visible in `runtime_environment`: a stale cache starts Nix,
and any launch or non-zero `nix develop` result is returned as a command error;
the validated stale payload is not retained as a fallback candidate.

## Conflict Check

- `2026-05-09-native-build-shell-ergonomics.md` deliberately left persistent
  GC roots and profile management out of its initial cache scope. The new
  offline requirement supersedes that non-goal only for roots owned by each
  existing runtime profile; it does not add a general environment manager.
- Cache reuse must continue to validate referenced Nix store paths. A fallback
  with missing local paths cannot be launched safely.
- Runtime input changes must not be hidden. Falling back must be visible and
  must not re-key the old environment as though it matched the new inputs.
- `robo refresh` and `robo update` intentionally clear their documented runtime
  cache scope. Removing a profile's cache must also release its retention roots.
- Active shell refresh already keeps the current shell usable after an
  evaluation failure; this change must preserve that behavior.

No ledger conflict blocks a narrowly scoped last-working fallback and
profile-local Nix store retention.

## Scope

- Keep a validated stale cache payload available while attempting to evaluate
  changed runtime inputs.
- If starting Nix, evaluating the new runtime shell, or parsing its captured
  environment fails, replay the failure and visibly continue with the last
  working environment.
- Never promote a fallback environment to the new runtime input key.
- Persist both the launch-input key and final-environment key at cache-write
  time instead of recomputing a supposed cache key from newly changed files;
  keep the previous cache format readable for migration and fallback.
- Register the Nix store paths referenced by a successfully cached environment
  as indirect GC roots under that profile's `.robo-nix/` state.
- Reuse an existing root generation cheaply and remove superseded root
  generations only after the replacement cache is saved.
- Save successful prompt-time refreshes as the new last-working environment.
- Document the offline behavior and its cold-bootstrap, explicit-refresh, and
  `--sync` boundaries.

## Non-Goals

- Making a project that has never completed runtime setup work offline.
- Pretending changed or invalid runtime inputs were applied to a fallback.
- Providing missing uv package artifacts for an explicitly requested
  `--sync`.
- Retaining cache state after the user explicitly clears it with `robo refresh`
  or after `robo update` clears all runtime profiles.

## Verification

- [x] focused cache fallback, cache migration, and GC-root tests
- [x] controlled end-to-end `robo run` test with a stale cache and a local Nix
  shim returning `error: offline test: network unavailable`
- [x] local `nix-store --realise ... --add-root ... --indirect` probe confirms
  multiple retained paths produce the expected root symlinks
- [x] `cargo test`
- [x] `nix develop --command cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix flake check --no-update-lock-file`
- [x] `git diff --check`
