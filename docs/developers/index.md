# Developer Guide

Use this section when you are changing `robo-nix` itself.

## Orientation

- [Product Boundary](./overview.md): ownership rules, repo shape, and local development loop.
- [Architecture](./architecture.md): implementation layers and generated project model.
- [Repository Workflow](./repository.md): docs build, formatting, tests, and failure-mode notes.
- [Contributing](https://github.com/ausbxuse/robo-nix/blob/develop/CONTRIBUTING.md): contribution standards, AI usage, review expectations, and PR disclosure.

## Documentation Boundaries

- User docs explain setup, workflow, runtime topics, and troubleshooting.
- Developer docs explain architecture, contracts, metadata, validation, and maintenance.
- README gives the public summary; avoid duplicating full guides there.

## Design Contracts

- [CLI UX Contract](./cli-ux.md) for command output, colors, progress, and wording.
- [Runtime Capability Model](./runtime-capability-model.md) for scalable runtime inference design.

## Iteration

`robo-nix` should evolve from validated downstream usage, not from a standing plan. Keep current limits and TODOs next to the page or module they affect.

Use [UX Design Notes](./ux-iteration.md) for durable CLI direction and iteration rules.
