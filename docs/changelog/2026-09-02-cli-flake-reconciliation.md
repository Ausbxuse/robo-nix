# CLI and Flake Reconciliation

## Context

Running the current CLI updater from the `robo-nix` source checkout reproduced
the reported failure:

```text
$ ROBO_NIX_NO_SPINNER=1 cargo run --quiet -- update
updating robo-nix flake input
error: flake.lock does not contain a locked `robo-nix` input
hint: rerun `robo update`; generated robo project flakes define inputs.robo-nix.
```

The source flake is the `robo-nix` input itself, so its lock correctly has no
child input named `robo-nix`. The update implementation only understood the
downstream-project layout.

A separately installed `robo` binary can also be newer than a downstream
project's locked `robo-nix` revision. The first runtime command currently uses
that older lock without reconciling the two halves of the tooling.

## Review Ledger

Potential conflict: automatic lock updates can add network latency, break
offline startup, or turn robo into a general flake updater.

Resolution: official builds carry their source revision and commit timestamp.
Before `robo shell` or `robo run`, robo may update only the project's existing
GitHub `robo-nix` input, and only when the running CLI's source commit timestamp
is later than the locked revision's. The attempt follows the source already
declared by the project, is recorded once per CLI/lock pair, and is best
effort. A failed attempt warns and continues with the existing lock, leaving
the last-working runtime cache available.

Potential conflict: updating from the source checkout could update unrelated
root-flake inputs or require a nonexistent downstream lock node.

Resolution: source-checkout `robo update` does not update the root lock or Git
checkout. It reinstalls the CLI from `.#robo`. Downstream `robo update` retains
its narrow behavior: update only the `robo-nix` input, reinstall from the
resulting locked source, and clear downstream runtime cache state.

## Change

- Teach `robo update` to distinguish a downstream robo project from the
  `robo-nix` source checkout.
- Embed source revision metadata in local Cargo, Nix, and release builds.
- Reconcile an older official project lock on the first runtime command for a
  new CLI/lock pair without making offline failure fatal.
- Preserve existing runtime caches during automatic reconciliation so normal
  last-working fallback remains available.
- Make progress child completion idempotent so a source-checkout reinstall is
  rendered once when the update tree finishes.
- Document both update paths and the automatic compatibility step.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] source-checkout `robo update` profile smoke
- [x] generated-project older-lock reconciliation smoke
- [x] automatic-update failure continues into cached runtime
- [x] progress-tree double-completion regression test
- [x] `npm --prefix docs run build`
- [x] parsed `.github/workflows/release.yml` with PyYAML
- [x] `nix flake check --no-update-lock-file`
- [x] `nix flake check --no-update-lock-file "path:$PWD"`
- [x] `git diff --check`
