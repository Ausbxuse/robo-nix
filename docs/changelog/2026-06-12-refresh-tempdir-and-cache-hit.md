# Refresh Tempdir And Cache Hit

## Context

Two downstream issues were reported from a runtime shell on an NVIDIA host:

```text
robo refresh
cleared .robo-nix runtime state
active shell refresh requested
runtime inputs changed in /home/nvidia/dexmate-teleop
  ✓ refresh requested manually
  ! changed env:LD_LIBRARY_PATH
--- shell refresh stderr ---
warning: ignoring the client-specified setting 'narinfo-cache-negative-ttl', because it is a restricted setting and you are not a trusted user
warning: ignoring the client-specified setting 'trusted-public-keys', because it is a restricted setting and you are not a trusted user
building '/nix/store/...-impure-nvidia-version-file.drv'...
error: creating temporary file '/tmp/nix-shell.Nnsk7z/nix-shell.sLE1RV': No such file or directory
error: failed to refresh shell environment; nix develop exited with exit status: 1
```

The same project also reported that a second `robo shell` still took close to a
minute instead of feeling like a runtime cache hit.

## Review Ledger

Related prior concerns:

- `2026-05-25-robo-owned-nix-cache-options.md` intentionally added direct
  robo-owned cache options to Nix commands.
- `2026-05-11-prompt-refresh-final-environment-key.md` made active shell
  fingerprints describe the final launched environment.

No conflict blocks this change. Robo-owned Nix commands can keep passing cache
options while hiding the known restricted-setting warnings in replayed failure
output, and final-environment fingerprints should not include ephemeral
`nix-shell` temp directories.

## Change

- Do not export or cache captured temp-directory variables from the evaluated
  runtime shell. A cached or refreshed shell should fall back to the host
  default temp location instead of preserving a deleted `/tmp/nix-shell.*`
  directory.
- Remove temp-directory variables from robo-owned Nix command environments so
  active prompt refresh does not ask Nix to create files under a stale runtime
  shell temp directory.
- Use the same workspace flake reference for active refresh as normal runtime
  setup, including `path:<workspace>` when required for untracked runtime input
  files.
- Filter the known restricted-setting warnings from replayed Nix stderr so
  failures show the actionable error first.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
