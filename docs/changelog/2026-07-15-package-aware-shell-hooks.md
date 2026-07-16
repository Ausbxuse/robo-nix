# 2026-07-15 - Package-Aware Shell Hooks

## Goal

Let project runtime hooks reference packages from robo's pinned nixpkgs set
without runtime path discovery, while preserving existing string hooks.

## Conflict Check

- `robo.nix` remains user-editable and canonical for runtime shell policy.
- Nix owns native tools, libraries, and the runtime shell environment.
- Keep the manifest surface focused on robot-learning runtime preparation.

No review-ledger conflict blocks allowing the existing `shellHook` field to use
the same `pkgs:` argument style as `extraPackages` and
`extraRuntimeLibraries`.

## Scope

- Accept either a string or `pkgs: string` for `shellHook`.
- Evaluate package-aware hooks with the same pinned package set as the rest of
  the selected profile.
- Reject hooks that resolve to another type with an actionable error.
- Document both forms and verify a package-backed environment value.

## Non-Goals

- Do not add another hook phase or hook ordering option.
- Do not change existing string-hook behavior.
- Do not infer packages from shell-hook contents.

## Verification

- [x] package-aware hook resolves `pkgs.zlib` to its Nix store path
- [x] function returning a non-string reports the `shellHook` type error
- [x] existing downstream string hook evaluates unchanged
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test` (110 passed)
- [x] `nix flake check --impure`
- [ ] `npm run build` under `docs/` (`vitepress` is unavailable because docs
      dependencies are not installed)
