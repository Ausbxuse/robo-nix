# robo-nix minimal core

This branch is a greenfield rebuild of `robo-nix`.

The first product contract is intentionally small:

- `robo init [path] [--force]` writes a uv-backed Nix project.
- `robo check` validates the files that `robo init` owns.
- `robo shell` enters `nix develop`.
- `robo run <command> [args...]` runs a command inside `nix develop`.

There is no `diagnose` command in this branch. Diagnostics should earn their way
back as small, reusable checks with clear ownership.
