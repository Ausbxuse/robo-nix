# CLI UX

`robo` output should feel calm, direct, and useful to people who want to work on robot learning, not learn Nix first.

The style is: one clear status line, short lowercase sections, sparse color, and enough evidence to debug without turning normal output into a log dump.

## Shape

Use one top-level status line with a themed prefix:

```text
robo: initialized simple
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
  robo activate
  uv sync
```

Do not mix heading styles such as `Project:`, `ok: Generated:`, and `robo: next steps:` in the same summary.

Generated shell summaries, including activation output from `nix develop`, should follow the same section and `key=value` shape. Do not print ad hoc labels such as `activated:`, `python:`, or `python packages:`.

Activation should enter the user's current interactive shell when possible. Keep this shell-agnostic: detect the parent shell executable, pass it through `ROBO_NIX_ACTIVATION_SHELL`, restore `SHELL` in the activated environment, and avoid shell-specific startup files or hardcoded prompt mutation. The runtime may export `ROBO_NIX_PROMPT_PREFIX` for users or shell integrations that choose to render it.

Because prompt mutation is intentionally shell-owned, `robo status` is the canonical way to see whether the current shell is activated. `robo deactivate` must not pretend a subprocess can exit its parent shell; it should print the clean `exit` action when inside an activated runtime.

Do not repeat command names as prefixes on every line. Prefer this:

```text
ok: workspace root exists
warn: uv virtual environment is missing
hint: run 'robo activate', then run 'uv sync' to create .venv
status=error issues=1 warnings=2
```

Avoid this:

```text
check: ok: workspace root exists
check: warn: uv virtual environment is missing
check: status=error issues=1 warnings=2
```

## Color

Color should guide scanning, not decorate every word.

- `robo:` and section headings: cyan/status color
- success markers such as `✓`: green
- warning markers such as `!`: yellow
- field labels and unchanged actions such as `kept`: dim
- generated actions: color the action word only, not the path
- quoted or backticked commands in human text: command color
- paths and bare commands: leave uncolored

Captured output must remain stable and grep-friendly. JSON output must remain raw machine-readable JSON with no labels or colors.

For key/value metadata such as `env=simple`, color only the key. Do not color the entire line.

For summary status lines, color each semantic value independently:

- `status=` key: dim
- `ok` value: green
- `error` value and issue count: red
- warning count: yellow

## Long Work

Use a progress bar when a command has known phases, such as deep runtime checks:

```text
robo: ⠹ [========>-----------] 2/4 preparing runtime
```

Use a spinner for a single long-running silent command, such as runtime bootstrap outside a larger progress flow. The spinner should use the same `robo:` prefix as status lines:

```text
robo: ⠹ checking runtime download plan
```

In non-interactive logs or `--debug` mode, print normal status lines instead of animated progress. Do not animate commands that already stream useful subprocess output.

## Icons

Use symbols only where they encode a state the user should understand quickly.

- `✓` means robo made a confident inference.
- `!` means robo saw evidence that needs user review.

Do not add icons to every section. Avoid emoji-style icons in CLI output; they render inconsistently and make logs noisier.

## Inference Severity

Separate confident inference from review-needed evidence:

```text
inferred
  ✓ Isaac Sim runtime            CUDA and graphics support

attention
  ! review component cuda-toolkit: workspace contains CUDA extension markers; src/foo.cu: CUDA source file
```

Use `inferred` for runtime choices robo applied. Use `attention` for weaker signals, skipped bootstrap scripts, missing project-owned paths, or cases where the runtime may be incomplete unless the user reviews the evidence.

Reserve red/error output for failures that stop the command.

## Next Steps

Next steps should be directly copyable shell commands. Keep them project-generic:

```text
next steps
  robo check
  robo activate
  uv sync
```

Color the whole command row as command text in interactive terminals, but do not add quotes, numbering, prompts, or icons.

Do not include speculative project commands such as `pytest`, dependency groups, extras, or source-specific install modes in generic init output.

## Wording

Prefer concrete nouns and short action verbs:

- `project`, `inferred`, `attention`, `generated`, `next steps`
- `wrote`, `kept`, `updated`, `skipped`

Avoid internal terms such as derivation names, Nix store paths, source URLs, or flake plumbing in normal output. Show those only under `--debug` or when needed to explain a failure.

For repeated successful checks, summarize instead of printing one line per item:

```text
ready
  ✓ runtime files
  ✓ Python contract
  ✓ inferred components
```

Print individual items only when the user needs to act on them, such as missing paths, unhealthy drivers, or mismatched components. Keep detailed evidence available through `robo check --verbose`; default `robo check` should read as a grouped health report with `project`, `ready`, `attention`, `skipped`, and `status` sections.
