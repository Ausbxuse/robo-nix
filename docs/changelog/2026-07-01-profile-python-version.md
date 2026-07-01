# Profile Python Versions

Runtime profiles can now set `pythonVersion = "3.10";` to select a different
CPython from nixpkgs-python without changing the workspace `.python-version`.
This is useful for vendor projects that still require Python 3.10 while the
main workspace runs newer Python.

Validation:

- `cargo test`
- `nix-instantiate --parse src/nix/project-flake.nix`
- `nix fmt -- --check src/nix/project-flake.nix`
