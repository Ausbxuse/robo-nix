# Python

Python is managed by `uv`, not by Nix.

`robo` expects generated projects to keep:

- `.python-version`
- `pyproject.toml`
- `uv.lock` when dependencies are resolved

Nix provides `uv` and native runtime libraries required by Python wheels.
