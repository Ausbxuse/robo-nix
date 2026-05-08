# CLI UX Contract

`robo` output should feel calm, direct, and useful to people who want to work on robot learning, not learn Nix first.

The style is one clear status line, short lowercase sections, sparse color, and enough evidence to debug without turning normal output into a log dump.

## Shape

Use one clear top-level status line. Color the action word, not a repeated tool prefix:

```text
initialized simple
```

After that, use plain lowercase section headings without trailing colons:

```text
project
  directory=.
  runtime=simple

inferred
  ✓ python 3.10                 pyproject.toml requires-python

generated
  wrote   ./flake.nix
  kept    ./pyproject.toml

next steps
  robo check
  robo shell
  uv sync
```

Do not mix heading styles such as `Project:`, `ok: Generated:`, and `next steps:` in the same summary.

## Shell Workflow

`robo shell` should launch a child runtime shell without requiring shell setup, dotfile edits, or hook installation. The shell prompt should show the `[robo]` marker by default, and plain `exit` should leave the runtime shell.

`robo shell` should prepare the runtime on demand when the shell cache is missing or stale. If runtime files are missing and stdin is interactive, it should ask before running `robo init` behavior and then continue into the shell. Non-interactive shell launches should fail clearly and point to `robo init .`. `robo init --build` may create runtime files and prebuild the runtime in one first-time setup command, but docs should keep `robo shell` as the normal path.

`uv sync` should be explicit:

- `robo build` must not ask to run it
- `robo build` must not provide an auto-sync flag
- `robo shell` must not install packages

`robo build` may cache realized runtime exports under `.robo-nix/` without entering a shell. The cache is an implementation detail for speed; `robo shell` and `robo run` should reuse it when runtime files still match and rebuild it clearly when the project contract changes.

## Color

Color should guide scanning, not decorate every word.

- phase labels such as `shell:` and section headings: cyan/status color
- success markers such as `✓`: green
- warning markers such as `!`: yellow
- field labels and unchanged actions such as `kept`: dim
- generated actions: color the action word only, not the path
- quoted or backticked commands in human text: command color
- paths and bare commands: leave uncolored

Captured output must remain stable and grep-friendly. JSON output must remain raw machine-readable JSON with no labels or colors.

## Long Work

Use a progress bar when a command has known phases. Use a spinner for one long-running silent command.

In non-interactive logs or `--debug` mode, print normal status lines instead of animated progress. Do not animate commands that already stream useful subprocess output.

Do not leave a spinner active while prompting for input. A hidden prompt looks like a hang.

For interactive shell setup, use a compact live tree with a parent setup line and one active child phase. Leave the completed tree behind:

```text
✓ robo ready 42ms
  └ ✓ shell: evaluating and realizing dev shell cached 1ms
  └ ✓ shell: launching shell 0ms
```

Long Nix phases may show a small rolling set of dim detail rows under the active child:

```text
⠋ robo shell
  └ ⠋ shell: evaluating and realizing dev shell 1.2s
    evaluating file '/workspace/flake.nix'
    instantiated 'python3-3.11.15'
```

Do not imply background reload, file watching, or hot-swap behavior unless that product surface exists.

## Inference Severity

Separate confident inference from review-needed evidence:

```text
inferred
  ✓ Isaac Sim runtime            CUDA and graphics support

attention
  ! review component cuda-toolkit: workspace contains CUDA extension markers
```

Use `inferred` for runtime choices `robo` applied. Use `attention` for weaker signals, skipped bootstrap scripts, missing project-owned paths, or cases where the runtime may be incomplete unless the user reviews the evidence.

## Next Steps

Next steps should be directly copyable shell commands:

```text
next steps
  robo check
  robo shell
  uv sync
```

Do not include speculative project commands such as `pytest`, dependency groups, extras, or source-specific install modes in generic init output.
