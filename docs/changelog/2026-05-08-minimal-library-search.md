# 2026-05-08 - Minimal Library Search

## Goal

Restore the useful `robo search` workflow from the original CLI without bringing
back the larger diagnostics surface.

## Scope

- Add `robo search <library>`.
- Normalize common missing-library fragments such as `/path/libGL.so.1:` to the
  basename `libGL.so`.
- Try local `nix-locate` first for speed and offline use.
- Fall back to the prebuilt `nix-index-database` flake only when local
  `nix-locate` is unavailable or its index is missing.
- Print deduped package candidates and a minimal `extraRuntimeLibraries`
  snippet.
- Keep `robo search` lookup-only. It does not edit `robo.nix`.

## Non-Goals

- No project mutation.
- No `robo check`, `robo init`, or `robo diagnose`.
- No Python package solving, dependency-group inference, or registry of package
  presets.
- No full Nix package search UI.

## Review Notes

Pending concerns:

- The prebuilt nix-index fallback may require network or flake evaluation on a
  cold host. That is still a fallback; the fast path remains local
  `nix-locate`.
- Candidate ranking is deliberately simple: dedupe and sort by Nix attribute.
  If real users need better ranking, add data from observed searches rather
  than hardcoding robotics package preferences.

## Verification

Run for this change:

- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `npm --prefix docs run build`
- fake `nix-locate` smoke for `robo search libassimp.so`
