# 2026-05-09 - Public Docs Consistency

## Goal

Keep public docs aligned with the current runtime behavior after the master
branch and GitHub Pages deployment changes.

## Conflict Check

- The public command surface remains `robo shell`, `robo run`, and
  `robo search`.
- Historical iteration notes remain excluded from the public VitePress build.
- Docs should explain current behavior without adding new product surfaces.

## Scope

- Align user docs with first-bootstrap inference from local uv sources and
  existing `uv.lock` package names.
- Align runtime component docs with current `native-build` and `desktop-gl`
  library coverage.
- Document the GitHub Pages deployment expectations for `master`.
- Make the VitePress favicon path respect the configured docs base path.

## Verification

- [x] `npm --prefix docs run build`
- [x] `nix develop --command cargo fmt --check`
- [x] `git diff --check`
