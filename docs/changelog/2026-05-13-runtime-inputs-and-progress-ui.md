# 2026-05-13 - Runtime Inputs And Progress UI Follow-Up

## Concern

The current runtime freshness key is based on a fixed set of project files plus
runtime-affecting environment variables. That is fast, but it can be untruthful
when `robo.nix` imports helper Nix files. A later change should make runtime
input tracking durable enough to include observed or declared runtime inputs
without running Nix on every prompt.

The command progress UI can also become more useful without adopting devenv's
full terminal interface. Robo should keep the original-style nested progress
tree, hide successful Nix output, and add only concise phase/detail rows that
make long setup waits understandable.

## Conflict Check

- Keep `robo shell`, `robo run`, and `robo search` as the only public command
  surface.
- Keep runtime refresh at the next prompt; do not add a background daemon or
  full-screen terminal UI.
- Keep successful Nix output hidden and preserve plain non-interactive output.
- Keep `robo.nix` user-owned after first creation.

No review-ledger conflict blocks better input tracking or clearer progress
status inside the existing runtime shell flow.

## Next Scope

- Track runtime inputs beyond the hardcoded file list, especially local Nix
  files imported by `robo.nix`.
- Keep cache-hit validation cheap: read the previous input list, check file
  metadata/content as needed, validate referenced store paths, and avoid Nix
  evaluation unless an input changed or the cache is missing.
- Improve progress detail rows for long `robo shell` and `robo run` setup while
  preserving plain non-interactive output.

## Non-Goals

- No new public commands.
- No full-screen TUI, process manager, task runner, services, or profiles.
- No Python dependency resolution or `uv sync` ownership change.
- No rewriting existing `robo.nix` during shell preparation or refresh.

## Change

- Runtime input fingerprints now include common local `.nix` imports found from
  `flake.nix` and `robo.nix`, including `default.nix` for directory-style Nix
  imports.
- Successful runtime evaluations now persist safe observed local Nix files from
  Nix's evaluated-file diagnostics when those diagnostics are available.
- Runtime refresh notices now report removed inputs as changes instead of only
  comparing the current input set.
- Runtime cache handling now distinguishes missing, stale, invalid, and
  missing-store-path cache states.
- Runtime cache progress labels now use neutral user-facing states, so a first
  launch says `new` instead of `missing` while debug output keeps the detailed
  cache reason.
- The nested progress tree now starts with a compact `runtime cache` phase and
  uses `evaluating runtime shell` as the Nix evaluation phase label.
- Runtime closure size estimation is debug-only, so first-run `robo shell`
  starts visible runtime progress instead of running silent Nix preflight
  commands before the timer appears.
- Nix runtime evaluation now asks for raw log output and turns common activity
  lines into concise live progress details such as planned fetches, builds,
  store copies, and cache fetches. Successful Nix logs stay hidden; failures
  still replay the captured Nix output.
- `robo --version` and `robo -V` now report the installed CLI version as
  standard global utility flags.
- Failed active-shell refresh attempts leave the old freshness key in place, so
  the next prompt retries refresh instead of treating the shell as updated.
- `.robo-nix/last-run.json` now has schema version 2 and includes typed host
  CUDA and host graphics probe summaries alongside the existing decision lines.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix develop --accept-flake-config --command cargo fmt --check`
- [x] `git diff --check`
