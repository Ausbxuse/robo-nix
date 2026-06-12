# CI Git Check Input

## Context

GitHub Actions `nix flake check` failed while building
`checks.x86_64-linux.default`:

```text
---- shell_refresh::tests::active_refresh_uses_workspace_flake_ref stdout ----
thread 'shell_refresh::tests::active_refresh_uses_workspace_flake_ref' panicked:
called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }

---- shell_refresh::tests::failed_refresh_does_not_poison_next_prompt_retry stdout ----
called `Result::unwrap()` on an `Err` value: PoisonError { .. }
```

The first failure happened because the Nix package test environment did not have
`git` on `PATH`; the second was lock poisoning after that panic.

## Review Ledger

Related prior concern:

- `2026-06-12-runtime-git.md` added Git as a runtime shell tool and added a
  Git-dependent unit test for workspace flake references.

No conflict blocks adding Git to package check inputs. The test intentionally
exercises Git worktree behavior, so the package test environment should provide
Git instead of skipping the test.

## Change

- Add `pkgs.git` to the Rust package's native check inputs so
  `nix flake check` can run Git-dependent unit tests in the Nix build sandbox.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix build .#checks.x86_64-linux.default --no-link`
