# 2026-05-20 - Untracked Runtime Files

## Concern

In a Git worktree, first bootstrap can generate `.python-version`, `flake.nix`,
and `robo.nix`, then immediately fail during `nix develop` because Nix treats
the current directory as a Git flake source and excludes untracked files:

```text
error: path '/nix/store/...-source/robo.nix' does not exist
```

Reproduced with a temporary Git repository where `flake.nix` was tracked and
`robo.nix` was present but untracked. `nix eval path:/tmp/project#...` can read
the same untracked `robo.nix`, confirming that an explicit path flake reference
keeps the local runtime files visible without mutating the Git index.

## Conflict Check

- Do not run `git add` or otherwise mutate the user's index.
- Do not overwrite a non-robo `flake.nix`.
- Keep generated project `flake.nix` minimal and keep `robo.nix` user-managed
  after first creation.
- Preserve normal Git-flake evaluation when runtime files are tracked.

No review-ledger conflict blocks choosing a path flake reference only when the
robo runtime input files are untracked.

## Scope

- Detect Git worktrees where `.python-version`, `flake.nix`, or `robo.nix` are
  present but not tracked.
- Use `path:<workspace>` for `nix develop` and debug closure estimation in that
  state so first bootstrap can continue.
- Keep the existing `.` flake reference when the runtime files are tracked or
  the workspace is not a Git worktree.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] temporary Git project with untracked `robo.nix` enters `robo run` without
  requiring `git add`
