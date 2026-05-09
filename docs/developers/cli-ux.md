# CLI UX

Human output should be concise, stable, and easy to paste into review.

## Labels

Use short labels for status lines:

```text
generated flake.nix
inferred  desktop-gl from pyproject.toml dependency `mujoco`
note      MuJoCo commonly needs desktop graphics runtime libraries.
error     missing .python-version
hint      choose the project Python version first, for example with `uv python pin <version>`.
debug     wrote .robo-nix/last-error.log
```

Labels may be colored in terminals. Captured output must remain plain text with
the same words and no escape codes.

## Spinners

Spinners are only for bounded command runs such as `robo run <command>`.
Interactive `robo shell` must not animate after the shell has started.
Subprocess output may supersede a spinner, but final robo-owned errors must be
printed after the spinner is cleared.

Disable animated output when stdout or stderr is not a terminal, when `NO_COLOR`
is set, or when `ROBO_NIX_NO_SPINNER=1` is set.

## Wording

Describe ownership boundaries directly:

- uv owns Python dependency sync.
- Nix owns native/runtime libraries.
- `robo.nix` is user-managed after first bootstrap.
- Host CUDA drivers are host-owned.
