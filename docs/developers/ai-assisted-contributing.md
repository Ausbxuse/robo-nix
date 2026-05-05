# AI-Assisted Contributing

`robo-nix` was created and developed with AI assistance. That provenance is part of the project, and the project should be open about it.

AI tools are allowed, including coding agents that inspect the repository, edit files, and run validation commands. The standard is simple: AI-assisted changes should be understandable, maintainable, and verified.

## When Agents Help

Good agent tasks are bounded and verifiable:

- tracing a failure through Rust, Nix, shell, and docs
- updating tests with a narrow reproducer
- checking whether docs match current behavior
- doing repetitive rename or wording cleanup
- comparing a proposed change against the project boundaries

Use extra care when asking agents to:

- propose new product modes without maintainer direction
- add broad abstractions because the code looks large
- infer robotics package policy from one downstream repository
- change generated project files by hand instead of source metadata or CLI generation
- treat an obvious-looking edit as a substitute for verification

## Prompt Shape

Give the agent concrete facts and a narrow goal:

```text
Goal: fix `robo run` so project bootstrap runs before the command.

Observed failure:
<paste command and full output>

Constraints:
- uv owns package sync; do not run `uv sync` implicitly.
- Keep runtime inference in metadata.
- Run the narrowest validation that proves the fix.
```

For docs work:

```text
Goal: update the README and developer docs to explain AI-assisted contributions.

Constraints:
- Keep the tone factual.
- Do not claim support that is not validated.
- Link to related projects without positioning them as competitors.
```

## Review Checklist

Before accepting agent output, check:

- Does every changed line serve the stated goal?
- Did the agent preserve the uv/Nix/Rust ownership boundary?
- Did it avoid generic `utils.rs`, broad registries, and single-use abstractions?
- Did it update the canonical doc page instead of adding overlapping docs?
- Did it run focused verification and report anything it could not run?
- Are generated files changed only through the generator or source metadata?

## Required Disclosure

Pull requests that used AI should say so briefly. A useful note is enough:

```text
AI-assisted: yes. Used an agent to trace the failing check and draft the patch.
Reviewed manually. Verified with `bash tests/dev-check.sh`.
```

This helps reviewers focus on assumptions, generated code, docs claims, and test coverage.

## Maintainer Rule

AI may write code and documentation. Humans own the merge.

If a change cannot be explained, reviewed, and validated without trusting the model, it needs more work before it is ready for `robo-nix`.
