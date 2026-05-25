# Gitignore Runtime State

## Context

First bootstrap creates `.robo-nix/` for runtime cache, diagnostics, shell
startup files, locks, and other robo-owned local state. In Git worktrees where
`.robo-nix/` was not already ignored, that directory appeared as untracked
project noise immediately after bootstrap.

## Review Ledger

Related prior concern:

- `2026-05-20-untracked-runtime-files.md` avoided mutating the Git index and
  used a path flake reference when runtime inputs were untracked.

No conflict blocks updating `.gitignore`: this change does not run `git add`,
does not rewrite tracked runtime files, and does not overwrite existing ignore
rules.

## Change

- During first bootstrap in a Git worktree, append `.robo-nix/` to the
  workspace `.gitignore` when the final relevant ignore rule does not already
  ignore it.
- Preserve existing `.gitignore` contents, including files without a trailing
  newline.
- Do not create `.gitignore` outside Git worktrees.

## Verification

- [x] `cargo test`
- [x] `nix develop --command cargo fmt --check`
- [x] `nix-instantiate --parse flake.nix`
