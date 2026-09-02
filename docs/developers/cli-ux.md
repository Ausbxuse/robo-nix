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
  ✓ linux-headers  pyproject.toml dependency `evdev`
    evdev native extensions include Linux input headers.
```

Color only the scanning anchors in terminals:

- section headings and active status labels: cyan and bold
- success marker `✓`: green and bold
- attention marker `!`: yellow and bold
- field or action words such as `wrote`: dim
- quoted or backticked commands: green

Captured output must stay plain text with no escape codes.

## Status

Use direct status text for active work. Add a phase only when it disambiguates
otherwise similar concurrent work:

```text
evaluating runtime shell
launching zsh
```

In terminals, long silent work uses the original nested progress tree. The
parent line names the command, the active child names the current phase, and
short dim details may appear under the active phase when Nix emits useful
progress:

```text
⠋ robo shell
  └ ⠋ evaluating runtime shell          2 packages    812ms
    copying '/workspace/' to the store
```

Leave a completed tree behind when the bounded setup phase succeeds:

```text
✓ robo ready                                            42ms
  └ ✓ evaluating runtime shell          cached         13ms
```

Do not animate while a child process is producing useful output. For `robo
shell`, capture the runtime shell environment with the tree first, then launch
the interactive shell directly without an active tree.

Disable animated output when stderr is not a terminal, when `NO_COLOR` is set,
when `ROBO_NIX_DEBUG=1` is set, or when `ROBO_NIX_NO_SPINNER=1` is set.

## Shell Launch

`robo shell` launches the user's default interactive shell. Selection order:

- `ROBO_NIX_SHELL`
- `$SHELL`, unless it points at generic Nix Bash or plain `sh`
- the login shell from `/etc/passwd`
- the parent interactive shell
- `zsh`, `bash`, `fish`, then `sh` from `PATH`

Interactive shells show a `[robo:<profile>]` prompt prefix by default. The
profile segment uses the active runtime profile selector, including `default`.
The prefix is injected through temporary startup files under
`.robo-nix/shell-startup/`; it does not edit user dotfiles. The prefix is added
to the user's existing prompt. `robo` does not invent a project-name prompt.

The same startup files run a prompt-time freshness check. If `flake.nix`,
`flake.lock`, `.python-version`, `pyproject.toml`, `uv.lock`, `robo.nix`, or
the selected runtime profile changes while the shell is active, `robo` reports
the changed inputs, re-evaluates the runtime shell, and exports the refreshed
environment into the current shell. This refresh does not rewrite `robo.nix`
and does not run `uv sync`.

`robo shell --sync` and `robo run --sync` are explicit exceptions: they run
`uv sync --locked` after runtime environment capture and before launching the
interactive shell or child command.

Once the Nix runtime environment has been captured for a runtime input key,
subsequent `robo shell` and `robo run` attempts may show the evaluation step as
`cached` and skip re-running `nix develop`. Cache reuse must validate referenced
Nix store paths first. Cached paths are retained with profile-owned indirect GC
roots. If a stale cache is still locally valid and evaluating changed inputs
fails, replay the setup failure and visibly use the last working environment;
never save that fallback under the changed input key.

## Wording

Describe ownership boundaries directly:

- uv owns Python dependency sync.
- Nix owns native/runtime libraries.
- `robo.nix` is user-managed after first bootstrap.
- Host CUDA drivers are host-owned; `robo` may bridge a detected `libcuda.so.1`
  provider when the project needs it.
