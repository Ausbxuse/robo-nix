# Repository Guidance

`robo-nix` is a focused runtime environment tool for robot-learning projects.
It should be robust, easy to use, and narrow. Do not grow it into a general
environment manager.

## Current Product Shape

- `robo shell` is the canonical user command.
- `robo run <command> [args...]` uses the same runtime preparation path as
  `robo shell`.
- There is no `robo init`, `robo check`, or `robo diagnose` in the current
  branch.
- `robo shell` may create missing runtime files during first bootstrap, then
  enters `nix develop`.
- `.python-version` is required. `robo` does not choose a default Python
  version.
- `pyproject.toml` is owned by uv/project policy. `robo` must never create it.
- `robo.nix` is user-editable after first creation and is canonical for the
  shell.

## Ownership Boundaries

- uv owns Python package metadata, dependency groups, extras, virtualenv sync,
  and lockfiles.
- Nix owns the CPython interpreter, native tools, runtime libraries, and shell
  environment.
- Rust owns command UX, diagnostics, project-file preparation, and command
  wrapping.
- Runtime inference rules should live in data files, not hardcoded Rust
  conditionals.

## Iteration Rules

- Keep each iteration small enough to review line by line.
- Record review concerns in `docs/development/iteration-*.md` before turning
  them into code.
- Promote only durable operating rules into this file.
- Before starting an implementation iteration, check for conflicts in the review
  ledger and call them out.

## Editing Rules

- Keep generated text in checked-in template/resource files. Rust may embed
  those files with `include_str!`.
- Avoid large raw generated strings in Rust.
- Use comments only when they clarify ownership, incomplete behavior, or a
  future hazard. Prefer specific markers such as `NOTE`, `TODO`, `FIXME`,
  `WARN`, `BUG`, and `DEBUG`.
- Do not overwrite a non-robo `flake.nix`.
- Do not rewrite an existing `robo.nix` from `robo shell`.

## Verification

Use the narrowest useful checks first:

```bash
cargo test
nix-instantiate --parse flake.nix
```

When generated project files change, also render a temporary project and parse
its generated `flake.nix` and `robo.nix`.
