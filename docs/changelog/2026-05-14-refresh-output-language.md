# 2026-05-14 - Refresh Output Language

## Concern

Manual `robo refresh` output is too noisy and frames the expected refresh
request as a changed input warning:

```text
refresh: active shell will update at the next prompt
shell: runtime inputs changed in /workspace
  ! changed manual refresh request
```

That is technically accurate, but it makes the normal path look like a problem
and repeats command prefixes where the surrounding context is already clear.

## Conflict Check

- Keep the refresh command narrow: clear robo-owned state and request prompt-time
  refresh in active runtime shells.
- Keep active shell refresh through the existing prompt hook.
- Preserve plain non-interactive output; avoid introducing a TUI or background
  process.

No review-ledger conflict blocks clearer refresh wording.

## Scope

- Remove `refresh:` and `shell:` prefixes from the manual refresh status path.
- Report a manual refresh request as an expected refresh event, not a warning.
- Use runtime-shell wording for active prompt-time refresh progress.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] fake-project active `robo refresh` smoke showing unprefixed status lines
