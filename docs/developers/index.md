# Developers

This branch is a greenfield rebuild of the `robo-nix` product surface.

Keep the product small:

- `robo shell` prepares and enters the runtime.
- `robo run <command>` prepares the same runtime, then runs one command.
- uv owns Python package sync and project policy.
- Nix owns CPython, native tools, runtime libraries, and shell environment.
- Rust owns user-facing workflow, diagnostics, templates, and command wrapping.

Read next:

- [Developer Overview](./overview.md)
- [CLI UX](./cli-ux.md)
