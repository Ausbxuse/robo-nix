# 2026-05-09 - Docs Cleanup

## Goal

Make the checked-in docs read like the current rewrite branch instead of a mix
of product docs and changelog history.

## Conflict Check

No active review-ledger conflicts block this pass. Current durable constraints
are aligned:

- Keep `robo shell` as the canonical user command.
- Keep `robo run` on the same runtime preparation path.
- Keep `robo search` lookup-only.
- Do not document `robo init`, `robo check`, or `robo diagnose` as current
  commands.
- Keep docs Node tooling under `docs/`.
- End installer docs with the current `robo shell` workflow.

## Scope

- Clarify the public home page, user index, getting-started flow, runtime
  reference, and troubleshooting page.
- Keep `docs/changelog/` as the review ledger but exclude it from the public
  VitePress build.
- Fix stale path references after moving templates and metadata under `src/`.
- Simplify the public navigation: no Home link inside the start section, one
  user section, one developer section.
- Reduce top-level site navigation to `User` and `Developer`; keep runtime
  material as a user workflow topic with examples.
- Merge install details into Getting Started and remove the repetitive
  standalone install page from user navigation.
- Expand developer notes so they describe current branch behavior: command
  surface, bootstrap flow, generated resources, runtime inference, project Nix
  library, lookup-only search, active shell refresh, and verification.
- Add a user-facing "what to expect" section covering shell, run, search,
  first-bootstrap files, active shell refresh, and error logs.
- Align README and developer wording with the current env-capture launch path:
  successful Nix output is hidden, the environment is parsed from `env -0`, and
  the final shell or command is launched directly with that environment.
- Fix `evdev` examples to show both inferred components: `native-build` and
  `linux-headers`.
- Treat nonzero final shell or command exits as normal child exits instead of
  setup errors; this avoids misleading debug logs for user command failures.

## Non-Goals

- No new command documentation.
- No change to installer behavior.
- No reorganization of the source tree.

## Verification

Ran for this change:

- `npm --prefix docs run build`
