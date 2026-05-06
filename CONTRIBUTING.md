# Contributing

`robo-nix` is infrastructure for downstream robotics projects. Changes should make projects easier to enter, run, and debug; they should not push extra environment knowledge, workflow policy, or maintenance burden onto users.

## Contribution Standard

Changes should preserve or improve:

- downstream API stability
- explicit platform behavior
- reproducibility
- regression coverage
- maintainability
- documentation accuracy

## AI Usage Contract

This repository was created and developed with AI assistance. Future AI-assisted development is welcome when it is transparent, reviewable, and verified carefully.

The required standard is:

- each non-trivial change must have a clear technical justification
- each behavior change must have sufficient verification
- each abstraction must be defensible in review
- each documentation update must match actual repo behavior

Acceptable verification includes:

- regression tests
- validation scripts
- `nix flake check`
- targeted `nix eval` or `nix run` validation
- profiling output for performance-oriented refactors

Avoid:

- merging AI-generated edits without understanding them
- accepting plausible-looking Nix code without evaluation or tests
- adding abstractions that cannot be justified beyond “the AI suggested it”
- documenting behavior that was never validated

Good AI-assisted tasks are bounded and verifiable:

- tracing a failure through Rust, Nix, shell, and docs
- adding or updating a narrow reproducer
- checking whether docs match current behavior
- doing repetitive rename or wording cleanup
- comparing a proposed change against the project boundaries

Use extra care when asking agents to propose product modes, add broad abstractions, infer package policy from one downstream project, or change generated project files by hand.

Pull requests that used AI should say so briefly:

```text
AI-assisted: yes. Used an agent to trace the failing check and draft the patch.
Reviewed manually. Verified with `bash tests/dev-check.sh`.
```

AI may write code and documentation. Humans own the merge. If a change cannot be explained, reviewed, and validated without trusting the model, it needs more work before it is ready for `robo-nix`.

## Preferred Workflow

Before opening or merging a non-trivial change:

```bash
bash tests/dev-check.sh
bash tests/full-check.sh
```

For smaller changes, run the smallest targeted verification that proves the change is correct.
Use `tests/dev-check.sh` as the default local loop; use `tests/full-check.sh` before merging broader changes.

If AI was used for the change, treat verification as mandatory rather than optional.

## What To Change Centrally

Add things to `robo-nix` when they are reusable:

- components
- generator improvements
- validation improvements
- project initialization improvements
- maintainer tooling

Do not add every project-specific environment combination to the central preset catalog.

## Docs

If behavior changes, update the relevant docs:

- [README.md](./README.md:1)
- [docs/users/getting-started.md](./docs/users/getting-started.md:1)
- [docs/users/diagnostics.md](./docs/users/diagnostics.md:1)
- [docs/developers/architecture.md](./docs/developers/architecture.md:1)
- the nearest runtime, CLI, or developer page when documenting a limit or TODO

Documentation should describe verified behavior, not intended behavior.
