# Iteration 026 - Master Product Language

## Goal

Prepare the current runtime workflow for the master branch by removing
branch-specific wording from public docs, installer defaults, generated flake
defaults, and command errors.

## Conflict Check

- The public command surface remains `robo shell`, `robo run`, and
  `robo search`.
- `robo init`, `robo check`, and `robo diagnose` are not reintroduced.
- The development ledger remains historical and is excluded from the public
  VitePress build.

## Scope

- Point installer and generated-project fallbacks at the master branch.
- Make unsupported command errors read as product-surface errors, not
  branch-state errors.
- Remove branch-specific status copy from the README and user/developer docs.
- Replace project-specific debugging names with generic examples.

## Verification

- [x] `cargo build --no-default-features`
- [x] `cargo test --no-default-features`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] `npm --prefix docs run build`
- [x] `git diff --check`
- [x] public docs/source search for branch-specific rewrite wording
- [x] fresh `target/debug/robo check`, `init`, and `diagnose` command text
