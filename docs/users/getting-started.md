# Getting Started

`robo-nix` prepares the native runtime layer for robot-learning projects. uv
still owns Python packages and virtualenv sync.

Start from a Python project directory:

```bash
uv python pin <version>
robo shell
uv sync
```

`robo shell` requires `.python-version`. It creates the Nix runtime files when
they are missing, then enters the development shell.

`robo` does not create `pyproject.toml`. Use uv or your project tooling for
Python package metadata.

After first bootstrap, edit `robo.nix` for project runtime choices such as native
build tools or desktop graphics support.

Run a command inside the runtime with:

```bash
robo run <command> [args...]
```
