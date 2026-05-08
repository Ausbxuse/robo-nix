# robo-nix minimal core

This branch is a greenfield rebuild of `robo-nix` around a shell-centered
workflow for robot-learning projects.

The current product contract is intentionally small:

- `robo shell` prepares missing runtime files and enters `nix develop`.
- `robo run <command> [args...]` prepares the same runtime and runs one command.
- `.python-version` is required.
- `pyproject.toml` is owned by uv/project policy and is never created by `robo`.
- `robo.nix` is created only on first bootstrap; after that it is user-managed.

There is no `robo init`, `robo check`, or `robo diagnose` in this branch.

Typical workflow:

```bash
uv python pin <version>
robo shell
uv sync
```

Runtime inference is first-bootstrap only. If `pyproject.toml` exists when
`robo.nix` is missing, `robo shell` uses the small data file in
`metadata/runtime-inference.tsv` to choose initial runtime components.
