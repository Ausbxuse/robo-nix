# Metadata

These files are the data-driven extension point for `robo-nix`.

- `components.nix` describes reusable runtime components implemented in `../modules`.
- `profiles.nix` defines starter profiles exposed by `robo init --list-profiles`.
- `runtime-inference.nix` currently maps common Python packages, workspace paths, and bootstrap script markers to reusable components.

Prefer metadata changes when adding support for a new package or common workspace shape. Rust should stay generic: read the project, apply these rules, and explain the result.

The target design is capability-based rather than component-first. See
`docs/developers/runtime-capability-model.md`. New coverage should move toward
rules that infer runtime requirements, plus component metadata that declares
which requirements each component provides.

When adding a current component-first inference rule:

1. Match stable package names from `pyproject.toml`, lowercase.
2. Map only to existing components from `components.nix`.
3. Keep the rule broadly reusable; do not encode downstream dependency groups, extras, pins, install modes, or package indexes.
4. Write `note` as user-facing text. It appears in `robo init` output.
5. Add a focused regression assertion in `tests/regression-api.sh` or `tests/robo-init-validation.sh`.

When adding new runtime coverage that is not urgent compatibility work, prefer
the capability model instead: rules infer requirements, and components declare
`provides`.
