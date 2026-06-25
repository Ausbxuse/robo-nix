# Active Shell Run Cache Key

## Context

Inside an active `robo shell`, `ROBO_NIX_DEBUG=1 robo run true` reported:

```text
debug: runtime cache stale runtime inputs
runtime cache refresh
```

No project files had changed. The active shell had inherited final runtime
variables such as `ROBO_NIX_LIBCUDA_PATH` and `LD_LIBRARY_PATH` that were added
after launch-time Nix evaluation. The runtime cache was written under the
parent launch key, while `robo run` inside the active shell checked the cache
with a key computed from the final exported runtime environment.

## Review Ledger

Related prior concerns:

- `2026-05-11-prompt-refresh-final-environment-key.md` requires active shell
  fingerprints to describe the final launched environment.
- `2026-06-12-launch-env-settle.md` handles first-prompt metadata settling
  when shell startup edits runtime-key environment variables.

No conflict blocks this change. Cache reuse can accept the key derived from the
cached final runtime environment without weakening prompt refresh correctness.

## Change

- Let runtime cache reads accept either the stored launch key or the runtime
  input key derived from the cached environment payload.
- Keep cache writes unchanged so normal `robo shell` and `robo run` launches
  can still reuse the parent launch key.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
