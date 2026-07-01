# 2026-06-18 - Runtime Cache Debug Inputs

## Concern

Users can observe a runtime refresh even when obvious runtime files such as
`robo.nix`, `.python-version`, and `uv.lock` have not changed. The current debug
message only reports `stale runtime inputs`, which confirms a cache miss but
does not identify the input that changed.

## Conflict Check

- Keep `robo shell` and `robo run` on the existing runtime cache path.
- Do not broaden the runtime input set or make unrelated repo files invalidate
  the cache.
- Keep successful Nix output hidden.

No review-ledger conflict blocks adding debug-only detail for cache misses.

## Change

- Persist the runtime input fingerprint list next to the runtime environment
  cache.
- In debug mode, report the specific changed runtime input names when a runtime
  cache entry is stale.

## Verification

- [x] `cargo test`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `nix-instantiate --parse flake.nix`
