# CLI UX

Human output should follow the original `robo` CLI shape: one clear status
surface, sparse color, lowercase headings, and pasteable captured logs.

## Sections

Use lowercase section headings without trailing colons:

```text
generated
  ✓ wrote    ./flake.nix
  ✓ wrote    ./robo.nix

inferred
  ✓ native-build   pyproject.toml dependency `evdev`
    evdev builds native extensions for Linux input devices.
```

Color only the scanning anchors in terminals:

- section headings and `phase:` status labels: cyan and bold
- success marker `✓`: green and bold
- attention marker `!`: yellow and bold
- field or action words such as `wrote`: dim
- quoted or backticked commands: green

Captured output must stay plain text with no escape codes.

## Status

Use `phase: detail` for active work:

```text
shell: evaluating and realizing dev shell
shell: launching zsh
```

In terminals, long silent work uses the original nested progress tree. The
parent line names the command, the active child names the current phase, and
short dim details may appear under the active phase when Nix emits useful
progress:

```text
⠋ robo shell
  └ ⠋ evaluating and realizing dev shell 2 packages    812ms
    copying '/workspace/' to the store
```

Leave a completed tree behind when the bounded setup phase succeeds:

```text
✓ robo ready                                            42ms
  └ ✓ evaluating and realizing dev shell cached         13ms
```

Do not animate while a child process is producing useful output. For `robo
shell`, preflight the dev shell with the tree first, then launch the interactive
shell without an active tree.

Disable animated output when stderr is not a terminal, when `NO_COLOR` is set,
when `ROBO_NIX_DEBUG=1` is set, or when `ROBO_NIX_NO_SPINNER=1` is set.

## Shell

`robo shell` should launch the user's default interactive shell. Selection order:

- `ROBO_NIX_SHELL`
- `$SHELL`, unless it points at generic Nix Bash or plain `sh`
- the login shell from `/etc/passwd`
- the parent interactive shell
- `zsh`, `bash`, `fish`, then `sh` from `PATH`

Interactive shells should show the original `[robo]` prompt prefix by default.
The prefix is injected through temporary startup files under
`.robo-nix/shell-startup/`; it should not edit user dotfiles.

## Wording

Describe ownership boundaries directly:

- uv owns Python dependency sync.
- Nix owns native/runtime libraries.
- `robo.nix` is user-managed after first bootstrap.
- Host CUDA drivers are host-owned.
