# Diagnostics

`robo check` is the primary debugging surface.

It explains what `robo` observed, which layer owns the failure, and what command or project file to inspect next.

## Which Command To Use

Use this decision tree:

- The project seems broken right now: run `robo check`.
- The failure area is obvious: run a focused check such as `robo check graphics`.
- You need slower runtime probes: run `robo check --deep`.
- You already have a traceback, compiler log, or loader error: pipe it to `robo diagnose -`.
- You need component provenance: run `robo check --why`.

## Current Runtime Checks

```bash
robo check
robo check graphics
robo check native
robo check python
robo check cuda
robo check --deep
```

Use focused checks when the failing area is obvious. Use `robo check --deep` for slower runtime probes that may realize more of the Nix environment.

`robo check` reports:

- whether the project is ready enough to work
- Python environment alignment
- likely missing runtime components
- focused graphics, native, CUDA, Python, or ROS status
- one or two concrete next actions

It avoids pretending project policy is known. For example, it does not guess uv groups, optional extras, private package indexes, or editable source pins.

## Existing Error Logs

`robo diagnose` classifies an existing error log. It does not probe the machine and it does not apply fixes.

```bash
uv sync 2>&1 | robo diagnose -
robo diagnose build.log
robo diagnose --json build.log
```

Use it when you already have an error and want to classify the failure boundary. The first beta implementation intentionally matches only high-confidence runtime failure signatures from the [failure guide](./failure-guide.md).

`robo diagnose` reports:

- a stable diagnosis ID
- the owner boundary
- the distinctive matched phrases
- one or two next commands
- the matching failure-guide page

If no entry matches, it says so. That is intentional: weak matches should not be presented as fixes.

When `robo diagnose` is uncertain, it prints an agent handoff section. Use that section when asking an LLM or teammate for help. The useful context is:

- the exact command that failed
- the complete error log, not only one line
- `robo diagnose --json` output
- `robo check --deep` output
- relevant `robo.nix` and `pyproject.toml` snippets

Example:

```bash
<failing command> 2>&1 | tee /tmp/robo-error.log
robo diagnose --json /tmp/robo-error.log
robo check --deep 2>&1 | tee /tmp/robo-check.log
```

Ask the agent to classify the failure owner first: uv/Python, Nix runtime, host GPU/driver, project bootstrap, or project dependency policy.

For known failure signatures and ownership boundaries, use the [Runtime Failure Guide](./failure-guide.md).

## Runtime Provenance

`robo check --why` explains why components, required paths, bootstrap scripts, and suggestions are part of the resolved runtime.

```bash
robo check --why
robo check --why --json
```

Use `robo check --deep` for broad runtime probes.

## Bootstrap Failures

Project bootstrap scripts are project-owned code. Non-interactive `robo init` records discovered bootstrap scripts as review suggestions instead of enabling them automatically.

A project enables bootstrap only by adding scripts to the `bootstrap` block in `robo.nix` or by passing `--source-script` explicitly.

If bootstrap fails, fix the project script or its required environment variables rather than adding project-specific policy to `robo-nix`.
