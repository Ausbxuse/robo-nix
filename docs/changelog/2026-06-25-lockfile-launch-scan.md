# Lockfile Launch Scan

## Context

After removing the cached-launch `nvidia-smi` probe, `we-teleop` steady-state
cached `robo run true` was about half a second while a tiny generated project
was about 20 ms. The remaining difference comes from project-size bookkeeping.
`we-teleop` has a large `uv.lock`, and robo parsed it as TOML on every launch
only to collect static package names for diagnostics and inference evidence.

## Review Ledger

Related prior concern:

- `2026-05-09-uv-venv-targeting-and-lockfile-inference.md` allows reading
  existing `uv.lock` package names as static evidence and explicitly avoids
  dependency resolution.

No conflict blocks this change. A line scan over package headers is still
static lockfile evidence and avoids turning launch diagnostics into a full
lockfile parse.

## Change

- Collect package names from `uv.lock` by scanning `[[package]]` sections and
  their top-level `name = "..."` entries.
- Keep read-error diagnostics, but stop parsing the entire lockfile as TOML for
  routine launch metadata.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `we-teleop` cached runtime smoke with the checkout debug binary completes
      in about 0.06s
