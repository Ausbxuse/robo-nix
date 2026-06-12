# Launch Env Settle

## Context

After `robo refresh`, a downstream `robo shell` on the NVIDIA host performed
normal setup and then immediately refreshed again at the first prompt:

```text
✓ robo ready                                           18.5s
  └ ✓ runtime cache                      new             0ms
  └ ✓ prefetching runtime paths                         9.4s
  └ ✓ evaluating runtime shell                          9.2s
launching bash
runtime inputs changed in /home/nvidia/dexmate-teleop
  ! changed env:LD_LIBRARY_PATH
✓ robo ready                                            6.4s
  └ ✓ evaluating runtime shell                          6.4s
```

The shell startup templates source the user's shell rc before the first robo
freshness hook. If that startup edits runtime-key environment variables such as
`LD_LIBRARY_PATH`, the active shell metadata written by `robo shell` can be
stale before the first prompt.

## Review Ledger

Related prior concern:

- `2026-05-11-prompt-refresh-final-environment-key.md` requires active shell
  fingerprints to describe the final launched environment.

No conflict blocks this change. A one-time launch settle can update active
metadata to match startup-time environment edits without hiding file changes,
manual refresh requests, or missing store paths.

## Change

- Mark newly launched runtime shells as needing one prompt-time environment
  settle.
- On the first prompt only, if the only differences are `env:*` runtime inputs,
  update `ROBO_NIX_RUNTIME_INPUT_KEY` and `ROBO_NIX_RUNTIME_INPUT_FILES` in the
  active shell without re-running `nix develop`.
- Continue to run a real refresh for changed files, manual refresh requests, or
  missing managed store paths.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
