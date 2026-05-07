# Project Boundary

The goal is a clear, reusable native runtime standard for uv-managed robot-learning projects. If a change makes downstream users learn more Nix, debug more hidden shell behavior, or carry more project-specific policy in `robo-nix`, treat that as a warning sign unless it solves a common problem.

## Ownership

Keep the boundary strict:

- uv owns Python version selection, virtual environment creation/sync, Python packages, dependency groups, optional extras, editable sources, indexes, and `uv.lock`.
- Nix owns the CPython interpreter, native runtime dependencies, native libraries, CUDA/graphics/ROS/simulator tooling, compilers, and shell environment.
- The Rust `robo` CLI owns user-facing workflow, diagnostics, command wrapping, and generated runtime files.
- Runtime inference coverage should live in metadata, not compiled Rust logic.

Do not make Nix-managed Python packaging a first-class mode unless real users prove that need.

## Repository Shape

```text
crates/robo-cli/       Rust CLI and user-facing workflow
nix/modules/           reusable runtime components
nix/metadata/          component docs, starter profiles, inference rules
nix/mk-flake.nix       downstream flake generator
nix/repo-support.nix   repo checks, docs build, package wrappers
tests/fixtures/        downstream flake fixtures
docs/                  VitePress documentation source
```

## Development Loop

Use focused checks while iterating:

```bash
cargo test -p robo-cli
nix run .#repo-fmt -- --check
nix run .#repo-lint
```

For docs:

```bash
nix build .#docs
nix run .#docs-dev
```

For CLI behavior changes, read the current [CLI UX contract](./cli-ux.md) before editing command output. Use [UX design notes](./ux-iteration.md) for exploratory direction.

For contribution standards and AI-assisted work, read [Contributing](https://github.com/ausbxuse/robo-nix/blob/develop/CONTRIBUTING.md).

For broader validation:

```bash
bash tests/dev-check.sh
nix flake check
```

GPU validation requires a suitable NVIDIA host and should not be treated as a cheap edit-loop check.
