# Users

Start here if you want to run a robot-learning project with uv-managed Python
packages and a Nix-managed native runtime.

The current minimal workflow is:

```bash
uv python pin 3.11
robo shell
uv sync
```

`robo shell` creates missing runtime files on first use. After that, `robo.nix`
is the project-owned runtime manifest.

Read next:

- [Getting Started](./getting-started.md)
- [Runtime Components](./runtime.md)
- [Troubleshooting](./troubleshooting.md)
