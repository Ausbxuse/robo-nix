# Metadata

These files are the data-driven extension point for `robo-nix`.

- `components.nix` describes reusable runtime components implemented in `../modules`.
- `profiles.nix` defines starter profiles exposed by `robo init --list-profiles`.
- `runtime-inference.nix` maps common Python packages and lockfile markers to runtime requirements.

Prefer metadata changes when adding support for a new package or common lockfile marker. Rust should stay generic: read the project, apply these rules, and explain the result.

The model is capability-based rather than component-first. See
`docs/developers/runtime-capability-model.md`. Rules infer runtime
requirements, and component metadata declares which requirements each component
provides.

When adding inference coverage:

1. Match stable package names from `pyproject.toml`, lowercase.
2. Add `requires` entries for stable runtime requirements.
3. Add or update component `provides` entries when a Nix component satisfies a runtime-owned requirement.
4. Keep the rule broadly reusable; do not encode downstream dependency groups, extras, pins, install modes, or package indexes.
5. Write `note` as user-facing text. It appears in `robo init` output.
6. Keep `components` only as a compatibility fallback while older generated files and checks still understand component-first rules.
7. Add a focused regression assertion in `tests/regression-api.sh` or `tests/robo-init-validation.sh`.

Do not infer runtime policy from discovered bootstrap scripts. Bootstrap scripts
are project-owned code and should enter generated config only when the user
passes an explicit `--source-script` or edits `robo.nix`.
