# UX Design Notes

This page records durable CLI direction. It is not a user contract, a release checklist, or required reading for normal changes.

For binding behavior, use:

- [CLI UX Contract](./cli-ux.md) for command output, colors, progress, and wording.
- [Runtime Capability Model](./runtime-capability-model.md) for runtime inference and metadata boundaries.

## Goal

`robo` should feel obvious to a robot-learning user who does not want to learn Nix. Commands should answer the user's immediate question with enough structure to scan, debug, and share.

The normal command surface should stay small:

```bash
robo up
robo status
robo check
robo shell
robo run <command>
```

Support commands are useful only when the intent is genuinely different from the daily workflow:

```bash
robo diagnose [FILE|-]
```

## Project Boundaries

Keep ownership explicit:

- uv owns Python packages, virtual environments, project sync, dependency groups, optional extras, package indexes, editable sources, and `uv.lock`.
- Nix owns native runtime dependencies, compilers, CUDA, graphics, ROS, simulators, and shell environment.
- `robo` owns workflow, command wrapping, generated runtime files, diagnostics, and plain-language explanations.

Practical consequences:

- Do not run `uv sync` implicitly.
- Do not prompt interactively to choose project-owned dependency policy.
- Do not hide host GPU, display, hardware, or vendor SDK failures behind guessed environment mutation.
- Do not add project-specific runtime fixes to shared rules unless the problem is broadly reusable and documented.

## Output Shape

Human output should be short, stable, and grep-friendly. Prefer:

- a clear action label
- observed facts
- owner boundary
- next command
- enough underlying tool output to debug

Avoid:

- decorative prose
- false certainty
- raw ad hoc prefixes
- long status banners
- changing project files without saying what changed

Machine-readable output should stay raw JSON with no labels, colors, or progress text.

## Diagnostics

Use command intent to keep diagnostics understandable:

- `robo status` gives a short health read.
- `robo check` probes the current project and host runtime.
- `robo diagnose` classifies an existing error log.

When a diagnosis is uncertain, say so. A useful uncertain result names the likely owner and the facts needed next.

## Design Sources

The project should learn from established CLI and documentation practice:

- [Command Line Interface Guidelines](https://clig.dev/) for human-first output and machine-readable modes.
- [NO_COLOR](https://no-color.org/) for disabling terminal color.
- [uv project docs](https://docs.astral.sh/uv/guides/projects/) for Python project ownership.
- [Homebrew troubleshooting](https://docs.brew.sh/Troubleshooting) for support workflows centered on diagnostics.
- [Diataxis](https://diataxis.fr/) for separating tutorials, how-to guides, reference, and explanation.

Use those references as direction, not as a reason to add more surfaces.

## Iteration Rule

Prefer a small behavior improvement with a focused test over a broad redesign.

Before adding a command, flag, output section, or metadata field, ask:

- Does it answer a real user question?
- Is the owner boundary clear?
- Can it be validated cheaply?
- Will downstream projects need less setup knowledge because of it?
- Is this better as data or Nix metadata instead of Rust logic?

If the answer is unclear, keep the surface smaller.
