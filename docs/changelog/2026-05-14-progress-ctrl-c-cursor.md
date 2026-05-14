# 2026-05-14 - Progress Ctrl-C Cursor Restore

## Concern

Interrupting `robo shell` while the animated runtime progress tree is still
running can leave the terminal cursor hidden. The progress code hides the
cursor and restores it through normal Rust cleanup, but SIGINT terminates the
process before those destructors run.

## Conflict Check

- Keep the existing bounded `robo shell` and `robo run` progress tree.
- Keep successful Nix output hidden and non-interactive output plain.
- Do not add a new command, background process, or terminal UI mode.

No review-ledger conflict blocks scoped terminal cleanup for interrupted
progress rendering.

## Reproduction

With a temp project containing `.python-version` and a fake `nix` that prints a
runtime log line once per second, run:

```bash
env -u NO_COLOR PATH="$tmpdir/bin:$PATH" "$repo/target/debug/robo" shell
```

Press Ctrl-C while the runtime progress tree is animating.

Key error: the TTY output contained `ESC[?25l` when the progress tree started,
but no matching `ESC[?25h` after SIGINT, leaving the shell cursor invisible.

## Change

- Install a scoped terminal signal cleanup guard only while robo has hidden the
  cursor for spinner/progress rendering.
- On SIGINT, SIGTERM, or SIGHUP, write the cursor-show sequence directly to
  stderr before exiting with the conventional signal exit code.
- Restore the previous signal handlers when progress rendering finishes
  normally before launching the user's runtime shell or command.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] TTY fake-`nix` SIGINT smoke: output ends with `ESC[?25h` and exits 130
