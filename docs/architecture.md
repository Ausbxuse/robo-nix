# Architecture

`robo-nix` has three active layers:

- `crates/robo-cli`: command-line UX and subprocess wrapping.
- `nix/modules`: reusable runtime components and inference metadata.
- `nix/mk-flake.nix`: turns a project manifest into flake outputs.

The main boundary is ownership:

- uv owns Python packages and locks.
- Nix owns native libraries and shell state.
- `robo` explains and runs the workflow.

Keep new package/runtime coverage data-driven in `nix/modules` unless it needs CLI behavior.
