# Start Here

Use this section when you want to install `robo`, enter a project runtime, run commands, or debug environment failures.

Most users only need this flow from an initialized project directory:

```bash
robo shell
uv sync
robo run python -m pytest
```

`robo` prepares the Nix-managed Python interpreter and runtime. `uv` still selects the Python version and owns Python packages, dependency groups, indexes, editable sources, and `uv.lock`.

## Pages

- [Usage](./usage.md): install `robo`, start a new project, enter an existing repo, and run daily commands.
- [Troubleshooting](./troubleshooting.md): use `robo check`, classify common errors, and decide what owns the fix.
- [Runtime Support](./runtime.md): CUDA, graphics, ROS, simulator tooling, compilers, FFmpeg, and host-owned limits.

::: warning Early beta
`robo-nix` is early beta software. CLI wording, generated files, diagnostics, runtime coverage, and installer behavior may change. Review generated `robo.nix` and `flake.nix` before committing them, and pin versions before depending on it for a shared team workflow.
:::
