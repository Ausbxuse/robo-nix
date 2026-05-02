# Architecture

`robo-nix` has four active layers:

- `crates/robo-cli`: command-line UX and subprocess wrapping.
- `nix/modules`: reusable runtime component implementations.
- `nix/metadata`: component metadata, starter profiles, and inference rules.
- `nix/mk-flake.nix`: turns a project manifest into flake outputs.

The main boundary is ownership:

- uv owns Python packages and locks.
- Nix owns native libraries and shell state.
- `robo` explains and runs the workflow.

Keep new package/runtime coverage data-driven in `nix/metadata` unless it needs CLI behavior.

The scalable target is the [runtime capability model](./runtime-capability-model.md):
project facts infer runtime requirements, Nix components and host probes provide
capabilities, and `robo doctor` compares the two. Avoid growing direct
package-to-component heuristics as the primary extension mechanism.
