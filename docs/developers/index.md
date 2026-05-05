# Developer Guide

This section is for people maintaining `robo-nix` itself.

The project should stay boring internally and friendly externally. Rust owns the CLI and diagnostics, while Nix and metadata own runtime coverage.

## Orientation

Start with:

1. [Developer Overview](./overview.md) for product boundaries and the repo shape.
2. [Architecture](./architecture.md) for the implementation layers.
3. [Repository Workflow](./repository.md) for local checks, docs, and maintenance workflow.
4. [AI-Assisted Contributing](./ai-assisted-contributing.md) for agent-assisted changes.

## Documentation Boundaries

- User docs explain setup, workflow, runtime topics, and troubleshooting.
- Developer docs explain architecture, contracts, metadata, validation, and maintenance.
- README gives the public summary; avoid duplicating full guides there.

## Design Contracts

Read these before changing behavior:

- [CLI UX Contract](./cli-ux.md) for command output, colors, progress, and wording.
- [Runtime Capability Model](./runtime-capability-model.md) for scalable runtime inference design.

## Planning

Use [Roadmap](./roadmap.md) for open direction and release priorities. [UX Design Notes](./ux-iteration.md) records exploratory CLI direction and is not required reading for normal changes.
