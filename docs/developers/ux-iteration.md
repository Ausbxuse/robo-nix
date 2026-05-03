# UX Iteration Guide

This is a target design for the next cleanup pass. It is not a promise that every command already behaves this way.

The goal is a CLI that feels obvious to a robotics user who does not want to learn Nix. Commands should answer the question the user actually has, with only enough structure to make that answer easy to scan.

## Survey Basis

This design should follow proven CLI and diagnostics patterns instead of inventing a private style.

- [Command Line Interface Guidelines](https://clig.dev/) argues for human-first output, fast feedback, stderr for progress/errors, stdout for primary and machine-readable output, progress for long-running work, and explicit JSON surfaces.
- [NO_COLOR](https://no-color.org/) is the de facto convention for disabling ANSI color when users or tools need plain output.
- [Pixi](https://pixi.sh/dev/reference/cli/pixi/) is a good environment-tool benchmark: short verbs, global output controls, generated CLI docs, `run`, `shell`, and explicit workspace commands.
- [uv](https://docs.astral.sh/uv/guides/projects/) owns Python project synchronization and documents that `uv run` and `uv sync` keep Python environments aligned with `pyproject.toml` and `uv.lock`.
- [conda doctor](https://docs.conda.io/projects/conda/en/stable/commands/doctor.html) models health checks as named checks with JSON, verbose, dry-run, and fix capability metadata.
- [Homebrew troubleshooting](https://docs.brew.sh/Troubleshooting) makes `brew doctor` and config output part of the support workflow instead of hiding diagnostics behind installer magic.
- [AWS CLI output formats](https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-output-format.html) show the value of separating human-friendly output from programmatic JSON.
- [Kubernetes kubectl](https://kubernetes.io/docs/reference/kubectl/kubectl-cmds/) keeps separate verbs for facts, detailed inspection, events, and reference explanation. That supports the `status`/`check`/`diagnose` split.
- [Google SRE troubleshooting](https://sre.google/sre-book/effective-troubleshooting/) emphasizes separating symptoms from causes and avoiding false certainty.
- [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) show why stable schemas and names matter for machine-consumable diagnostics.
- [Sentry issue grouping](https://docs.sentry.io/platforms/dotnet/guides/apple/usage/sdk-fingerprinting/) is a useful model for grouping noisy failures by stable fingerprints instead of one-off string matching.
- [Diataxis](https://diataxis.fr/) separates tutorials, how-to guides, reference, and explanation. Robo docs should do the same instead of mixing onboarding, policy, and internals in one page.

The practical takeaway is:

```text
keep the normal command surface small
make diagnostics explicit and structured
separate human prose from machine output
use stable diagnostic IDs and schemas
do not mutate project policy from weak guesses
```

## Product Principles

Robo should optimize for people who want to work on robot learning, simulation, and robotics infrastructure, not people who want to debug Nix.

The hard boundary is:

```text
uv owns Python packages and virtual environments.
Nix owns native runtime dependencies.
robo owns workflow, diagnostics, and command wrapping.
```

Non-goals:

- no implicit `uv sync`
- no hidden project mutation
- no auto-running vendor or bootstrap scripts
- no LLM inside `robo`
- no guessed host GPU driver path mutation
- no project-specific fixes in shared runtime rules
- no weak diagnostic match presented as a certain fix

## Command Model

The top-level workflow should stay small:

```bash
robo up
robo status
robo check
robo shell
robo run <command>
```

`up` prepares the project. `status` gives the short read. `check` probes the runtime. `shell` enters the runtime. `run` executes one command in the runtime.

Support commands are allowed only when the user intent is clearly different from the core workflow:

```bash
robo diagnose [FILE|-]
robo bundle
```

`diagnose` classifies existing error logs. `bundle` collects support context. They are not part of the normal happy path.

`robo add <capability>` is a future runtime-editing command. If examples use `robo add qt6`, the command model must include it as future work, not as a hidden assumption:

```bash
robo add qt6
robo remove qt6
```

Avoid vague top-level verbs. In particular, do not add `robo next` as a first-release command. It creates hidden state questions: next from which diagnostic, from which run, and where is that state stored? Prefer `robo diagnose` printing next actions directly, and later add:

```bash
robo diagnose --last
robo diagnose --id native.qt6-missing
```

if persisted diagnostic state proves useful.

## Command Responsibilities

Use this distinction consistently:

| Command | User question | Behavior |
| --- | --- | --- |
| `robo status` | "Can I work now?" | Fast summary. No deep probes. |
| `robo check` | "What is broken in this runtime?" | Active runtime checks and focused next action. |
| `robo check graphics` | "Why does display/OpenGL fail?" | Domain-specific probes and evidence. |
| `robo diagnose` | "What does this error log mean?" | Classify text against known failure entries. |
| `robo bundle` | "What should I attach to an issue or give an agent?" | Local support bundle with redaction. |
| `robo up` | "Prepare this project." | Generate/update runtime files and realize prerequisites. |
| `robo shell` | "Put me in the environment." | Enter the runtime quickly. |
| `robo run` | "Run this command in the environment." | Wrap one child process without hiding its useful output. |

This mirrors established tools without copying them blindly: Pixi has `run` and `shell`; conda and Homebrew have diagnostic health surfaces; kubectl separates short facts, detailed inspection, and explain-like reference.

## Check Shape

`robo check` should become the general runtime diagnostic entry point:

```bash
robo check
robo check graphics
robo check cuda
robo check python
robo check native
robo check ros
```

The plain command checks the common runtime contract and shows the most useful next step. Domain checks go deeper in one area without forcing users through a giant all-system report.

Suggested ownership:

- `robo check graphics`: display variables, Wayland/X11, GLX/EGL/GLVND, MuJoCo viewer assumptions, Qt/OpenCV graphics needs.
- `robo check cuda`: NVIDIA driver visibility, `libcuda.so.1`, CUDA wheel ABI, toolkit version, `nvcc`, CUDA extension build surface.
- `robo check python`: `.python-version`, `pyproject.toml`, `uv.lock`, `.venv`, interpreter origin, Python/native ABI mixing.
- `robo check native`: compiler, CMake, pkg-config, `libstdc++`, glibc boundary, native tool wheel shims.
- `robo check ros`: selected ROS distribution, workspace paths, colcon tooling, ROS package environment.

If a domain is not relevant to the current runtime, say so directly:

```text
Graphics runtime is not required by this project.
```

Do not pretend everything is checked when the project has not selected the relevant runtime components.

### Extending Checks

New tool checks should be cheap to add. A contributor should not have to touch Rust command wiring, Nix metadata, docs, fixture tests, and shell output snapshots for every `robo check <tool>` addition.

Prefer this shape:

- domain checks such as `graphics`, `cuda`, `python`, `native`, and `ros` own reusable probes
- tool checks compose those domains from metadata
- Rust is required only when a check needs a new reusable probe primitive
- docs explain what the check can prove, what it cannot prove, and which layer owns failures

For example, `robo check mujoco` should usually be metadata that says:

```text
title: MuJoCo
implies: python, graphics, native
python imports: mujoco
```

The generic check runner can report project signals, run the Python import probe, include graphics/native summaries, and point to `robo check graphics` when OpenGL context creation fails. It should not need a dedicated Rust module unless MuJoCo needs a reliable custom probe that cannot be expressed with existing primitives.

Avoid this shape:

- one Rust module per tool by default
- package-specific shell exports
- guessed driver path mutation
- uv group, extra, or dependency policy inferred for one downstream project
- output snapshots that make every new tool check expensive to maintain

This keeps `robo check <tool>` scalable: most tool support is data and documentation, while Rust stays focused on boring, reusable diagnostics.

## Diagnostic Output

The default `robo check <domain>` output should not look like a report. It should answer the user's question in the first line, then give the shortest useful explanation and the command to type next.

This follows CLIG's "say just enough" guidance and Google's troubleshooting distinction between symptom, probable cause, and evidence.

For a confident failure:

```text
Qt6 is missing from the robo runtime.

CMake looked for Qt6Config.cmake while building a native package.
Add Qt6, then refresh the runtime:

  robo add qt6
  robo up
```

For an uncertain failure:

```text
A native build failed, but robo could not identify one clear missing runtime.

The build used CMake. Common missing pieces include native tools,
pkg-config, Qt, graphics headers, or CUDA headers.

See the details:

  robo check native --verbose
```

For a healthy check:

```text
Native build support looks ready.

Found a compiler, CMake, pkg-config, and common C/C++ runtime libraries.
```

For an irrelevant check:

```text
Native build support is not required by this project.
```

Use the sectioned form only for `--verbose`, JSON, or full reports where users are explicitly asking for evidence. Default diagnostics should be natural and compact.

Verbose diagnostics can be structured:

```text
native
  result: blocked
  reason: Qt6Config.cmake missing

evidence
  build output mentioned CMake
  missing Qt6Config.cmake
  compiler available
  pkg-config available

ownership
  layer: Nix runtime dependencies

commands
  robo add qt6
  robo up
```

## Bootstrap Boundary

Bootstrap scripts are project policy. They are usually repo-specific, depend on local source layout, and may encode conventions that do not generalize across robotics projects.

`robo` should not infer project workflow from conventions such as:

- `scripts/bootstrap_*`
- `third_party/`
- project-specific helper functions
- custom environment variables
- local SDK checkout layouts

Instead, bootstrap should be explicit and opt-in. A project can declare that it wants bootstrap code to run, but `robo init` should avoid automatically discovering and enabling scripts from repo shape alone.

Good explicit surfaces:

```bash
robo init --bootstrap scripts/bootstrap_local_sdk.sh
```

or a project-owned config block:

```nix
bootstrap = ''
  ./scripts/bootstrap_local_sdk.sh
'';
```

The scalable `robo` responsibility is not to understand every bootstrap script. It is to provide the runtime capabilities those scripts need and to classify failures when they happen.

Prefer diagnosing reusable failure classes:

- CMake could not find Qt
- `pkg-config` is missing
- `nvcc` is missing
- `Python.h` is missing
- `linux/input.h` is missing
- `GL/gl.h` or `libGL.so.1` is missing
- a `.venv/bin` build-tool shim is mixing host glibc with Nix runtime libraries

The design principle is:

```text
Do not infer repo workflow.
Infer missing runtime capability.
```

This keeps `robo-nix` scalable when downstream projects use different layouts, package managers, vendored SDKs, optional extras, or bootstrap policies.

## Vendor Runtime Boundary

Third-party robotics vendors often ship non-standard repositories. They may have no `pyproject.toml`, local C/C++ source, Makefiles, CMake projects, binary blobs, custom install scripts, undocumented environment variables, or SDK folders in arbitrary paths.

`robo` should handle this gracefully without learning every vendor workflow.

The scalable rule is:

```text
Do not model vendors.
Model capabilities their code needs.
```

`robo` owns reusable runtime capabilities:

- `native-build`
- `graphics`
- `qt6`
- `media`
- `cuda-toolkit`
- `linux-headers`
- `ros2`
- `mujoco`
- future capabilities such as `libusb`, `udev`, or `vulkan`

Projects own vendor policy:

- which vendor script to run
- where local SDK source lives
- which optional Python groups or extras to install
- which private package indexes, credentials, or binary blobs are needed
- which vendor-specific environment variables are required
- whether a vendor bootstrap step should run automatically

Missing `pyproject.toml` should not be a hard error. A vendor repo can still use `robo.nix` as its runtime contract:

```text
robo is ready for this project.

No Python project manifest was found.
If this repo builds native code, add the runtime pieces it needs:

  robo add native-build
  robo check native
```

For a non-standard vendor checkout:

```bash
robo add native-build qt6 linux-headers
robo up --shell
./vendor/build.sh
```

If the vendor build fails, `robo check` should classify the reusable failure:

```text
Qt6 is missing from the robo runtime.

CMake looked for Qt6Config.cmake while building native code.
Add Qt6, then refresh the runtime:

  robo add qt6
  robo up
```

Avoid:

- scanning arbitrary `third_party/` conventions
- guessing vendor install scripts
- auto-running bootstrap scripts
- adding vendor-specific environment variables to generic components
- maintaining a central vendor matrix such as `vendor_x_sdk = qt6 + libusb + custom_env`
- treating missing `pyproject.toml` as a failed project

This keeps vendor support broad without turning `robo-nix` into a collection of repo-specific hacks.

## Failure Knowledge Base

`robo` should grow a small, maintained failure knowledge base for common runtime errors. This is different from pretending to solve every error automatically.

The goal is:

```text
make opaque runtime errors searchable
classify the owning layer
show one or two useful next actions
produce agent-readable diagnostics
link to deeper docs
```

Use two first-release surfaces:

```bash
robo diagnose [FILE|-] [--json]
robo bundle [--json]
```

`robo diagnose` classifies logs or explicitly recorded failures. `robo bundle` collects support facts for humans, CI, or external agents.

Do not start with fuzzy UI. Fuzzy search is useful later, but the first implementation should be structured diagnosis plus a good data model. Interactive fuzzy selection can sit on top after the entries, scoring, and JSON schema are reliable.

### Human Flow

If a command fails:

```bash
uv sync 2>&1 | robo diagnose -
```

Confident output:

```text
This looks like a Python/native ABI mismatch.

Your .venv may have been created outside the robo runtime and is loading
Nix native libraries on an older host glibc.

Try:

  robo shell
  uv venv --clear
  uv sync
```

If there is no input and no recorded failure:

```text
I need an error log to diagnose.

Run a failed command like this:

  <command> 2>&1 | robo diagnose -
```

If matching is ambiguous:

```text
I found a few possible causes.

1. Qt6 is missing from the robo runtime
2. Python-owned CMake helper may be missing
3. Native build support may be incomplete

Run a focused check:

  robo check native --verbose
```

If no known failure matches:

```text
I could not identify a known runtime failure.

Try:

  robo status
  robo check native --verbose
  robo check graphics --verbose
  robo bundle
```

### Agent Flow

Agents should not scrape human prose. They should ask for structured output:

```bash
uv sync 2>&1 | robo diagnose - --json
robo diagnose --id native.qt6-missing --json
robo bundle --json
```

Diagnostic JSON should include:

```json
{
  "schema": "robo.diagnostic.v1",
  "status": "blocked",
  "input_hash": "sha256:...",
  "matches": [
    {
      "id": "native.qt6-missing",
      "title": "Qt6 is missing from the robo runtime",
      "owner": "nix-runtime",
      "severity": "error",
      "confidence": "high",
      "matched": ["Qt6Config.cmake"],
      "summary": "CMake looked for Qt6 while building native code.",
      "next_actions": [
        {
          "command": "robo add qt6",
          "mutates": ["robo.nix"],
          "risk": "low",
          "requires_confirmation": true
        },
        {
          "command": "robo up",
          "mutates": [".robo-nix/"],
          "risk": "low",
          "requires_confirmation": false
        }
      ],
      "verify": ["rerun the failed project command"],
      "docs": "/users/failure-guide#qt-cmake-package-missing"
    }
  ]
}
```

Agent-facing output should be stable, compact, and replayable:

- schema version
- stable diagnostic IDs
- confidence
- owning layer
- matched snippets
- safe next commands
- mutation/risk metadata
- verification step
- docs URL
- input hash

This follows OpenTelemetry's stable naming lesson and Sentry's fingerprinting lesson: noisy logs should map to stable concepts that humans and tools can discuss repeatedly.

### Failure Entry Data

Use one source of truth for failure entries, for example:

```text
data/failures/*.toml
```

Each entry should include:

```toml
id = "native.qt6-missing"
title = "Qt6 is missing from the robo runtime"
owner = "nix-runtime"
severity = "error"
precision = "high"

patterns = [
  "Qt6Config.cmake",
  "Could not find a package configuration file provided by \"Qt6\""
]

negative_patterns = [
  "pybind11Config.cmake"
]

summary = "CMake looked for Qt6 while building native code."

actions = [
  { command = "robo add qt6", mutates = ["robo.nix"], risk = "low" },
  { command = "robo up", mutates = [".robo-nix"], risk = "low" }
]

docs = "/users/failure-guide#qt-cmake-package-missing"
```

Generate the docs page from this data later. Do not maintain two separate truth sources forever.

### Matching Rules

Keep matching boring:

- exact substring match is strong
- multiple related matches are stronger
- generic terms such as `cmake` alone are weak
- negative patterns reduce confidence
- never return a direct fix from weak confidence
- cap normal output to the top three matches
- order cascading failures carefully, such as Python/native ABI mismatch before downstream native build noise

Confidence behavior:

- `high`: print a direct diagnosis and next commands
- `medium`: print likely causes and ask for a focused check or more log context
- `low`: say no clear match and suggest domain checks

Huge logs should be bounded:

```text
Read the last 4000 lines of the log.
Use `--full` if the important error appeared earlier.
```

### Bundles

`robo bundle` should collect agent-ready support context into a local, gitignored directory:

```text
.robo-nix/bundles/2026-05-03T.../
  summary.md
  diagnostics.json
  runtime-contract.json
  host-facts.json
  logs/
  redactions.json
```

Include:

- `robo` version
- OS/platform
- Nix version
- selected runtime components
- `robo.nix` contract
- Python version and uv state summary
- CUDA/graphics facts when available
- recent command failure logs when explicitly included
- matched failure IDs
- exact next probes to run

`robo bundle --json` should print a small manifest, not the entire bundle:

```json
{
  "schema": "robo.bundle.v1",
  "path": ".robo-nix/bundles/2026-05-03T120000Z",
  "files": ["summary.md", "diagnostics.json", "runtime-contract.json"],
  "redactions": 8
}
```

Redact secrets by default. Redact tokens, credentials, URLs with embedded auth, and environment variables containing names such as `TOKEN`, `KEY`, `SECRET`, `PASSWORD`, `AWS_`, or `HF_TOKEN`. Show a redaction summary. Require an explicit flag for unredacted bundles.

### Boundaries

- no LLM inside `robo`
- no automatic fix on diagnose
- no hidden mutation
- no auto-running vendor scripts
- no pretending weak matches are certain
- no huge free-form prose in JSON
- no project-specific fixes in shared failure entries

Human output should be natural. Agent output should be structured. Both should come from the same diagnostic data.

## Python Sync Boundary

`robo` should not run `uv sync` implicitly, and it should not ask an interactive "run uv sync now?" prompt. Python package installation is outside the product boundary because projects often choose dependency groups, optional extras, package indexes, editable source layouts, and install modes deliberately.

This differs from uv itself. uv can auto-sync inside `uv run` because uv owns Python packages and the lockfile. Robo does not own those choices; it makes the native runtime usable for whatever uv command the project documents.

`robo` can say whether Python packages appear synced. It can explain that `.venv` is missing, `uv.lock` is stale, or a native extension build failed because runtime libraries are unavailable. It should then hand control back to the project-owned uv command.

Good action output:

```text
robo is ready for this project.

Python packages are not synced yet.
Run the uv command documented by this project.
Default: `uv sync`
```

With a project-declared hint, if that feature is added later:

```text
robo is ready for this project.

Python packages are not synced yet.
This project suggests:
  uv sync --extra sim --group dev
```

Avoid:

```text
Run `uv sync` now? [y/N]
```

and avoid:

```bash
robo up --sync
```

unless the product boundary is deliberately changed later. The safer rule is:

```text
robo prepares the native runtime.
uv installs Python packages.
the project documents its uv policy.
```

## Status Shape

`robo status` should be short. It should answer: is this project ready enough to work?

Default status should avoid a report-shaped layout:

```text
robo is ready for dexmate-teleop.

Python packages are not synced yet.
Run the uv command documented by this project.
Default: `uv sync`
```

When there is a blocker:

```text
robo is blocked for dexmate-teleop.

The runtime has no graphics support, but this project appears to use MuJoCo.
Run `robo check graphics` for the exact failure.
```

Use sectioned status only for `robo status --verbose`:

```text
project
  name: dexmate-teleop
  python: 3.11
  workspace: ~/src/dev/dexmate/dexmate-teleop

ready
  runtime files
  Python contract

attention
  Python environment missing
    run: uv sync

status
  ok, 1 warning
```

`status` should not print full debug evidence, provenance, long subprocess output, or every host probe. It can point to `robo check <domain>` when the user needs more.

## Section Discipline

The current sectioned output style is useful for summaries and reports, but it should not be forced onto every command.

Use sectioned output for:

- `--verbose` status
- verbose diagnostics
- generated file summaries when the user asked for detail
- multi-domain check reports
- machine-adjacent summaries where fields matter

Avoid sectioned output for:

- default `robo up`
- default `robo status`
- one-line success or failure
- shell cards that are already visual containers
- direct command wrappers
- prompts
- help text where the native Clap layout is clearer

For one clear result, prefer one clear line:

```text
ok: runtime ready
```

Do not turn every command into a help-menu-like block just for consistency. Consistency means the same kind of information has the same shape, not that every command has the same layout.

## Action Output

Action commands should feel like progress toward work, not like reports. Use short sentences and durable result lines. Keep the next step obvious.

Prefer this shape for `robo up`:

```text
Setting up robo for this project...

Created robo.nix.
Using Python 3.11.
Added graphics support for MuJoCo and OpenCV.

robo is ready for this project.
Run `robo shell` to enter the environment.
```

If nothing changed:

```text
robo is ready for this project.
Run `robo shell` to enter the environment.
```

If `--shell` was requested:

```text
robo is ready for this project.
Entering the environment...
```

Avoid report-shaped action output:

```text
setup
  created robo.nix
  detected Python 3.11

ready
  runtime files

next
  run: robo shell
```

That shape is useful for inspection commands, but it makes action commands feel like generated status reports. For `up`, `shell`, `run`, `add`, and `remove`, prefer natural result lines:

```text
Added qt6 runtime support.
Run `robo up` to refresh the environment.
```

Use "I noticed..." sparingly for lower-confidence observations:

```text
I noticed this project may use CUDA.
Run `robo check cuda` if GPU code does not work.
```

## Progress Rules

Progress UI should tell the user which layer is busy:

- generated files
- project bootstrap
- CUDA compatibility
- Nix shell evaluation
- shell environment capture

Use a progress bar only when the command has known phases. Use a spinner for one long-running silent subprocess. Do not use a spinner for commands that stream useful output.

Spinner and progress output is temporary narration. Final output is the durable result. A spinner should tell the user what is happening now; it should not become part of the final report.

While work is happening:

```text
Setting up robo for this project...
```

After it finishes, clear the spinner and print what changed:

```text
Created robo.nix.
Using Python 3.11.

robo is ready for this project.
Run `robo shell` to enter the environment.
```

For long work with distinct phases, prefer one progress line whose label changes:

```text
Checking project files...
Resolving Nix runtime...
Preparing Python tools...
Running project bootstrap...
```

Only use a progress bar when progress is real. A phase counter is usually better than a fake percentage:

```text
[2/5] Resolving runtime
[3/5] Checking graphics
```

Avoid visual bars when the percentage is guessed.

If `robo run` is explicitly wrapping a child process that already streams useful output, do not hide it behind a spinner. Print a short intro and stream the command:

```text
Running `uv sync` inside the robo environment...
```

Interactive and non-interactive behavior should differ:

- terminal: spinner or progress bar is fine
- non-terminal logs: plain status lines
- `--debug`: raw subprocess commands and output when useful

Never leave a spinner active while waiting for input. A hidden prompt looks like a hang.

In non-interactive logs, do not render spinner frames. Print plain phase lines to stderr:

```text
checking project files
resolving Nix runtime
preparing Python tools
ready
```

Good long-running labels:

```text
bootstrap: running project bootstrap
shell: evaluating and realizing dev shell
cuda: checking host driver
```

Bad long-running labels:

```text
setting up runtime
preparing
working
```

Vague labels slow down support because they hide the failing layer.

## Machine Output

Machine-readable output is a separate interface, not colored human prose with labels stripped out.

Rules:

- JSON goes to stdout.
- Progress, warnings, and human narration go to stderr.
- JSON never contains ANSI colors.
- JSON must be stable enough for agents and CI.
- Use schema versions, such as `robo.diagnostic.v1`.
- Respect `NO_COLOR` for human output.
- Support `--color auto|always|never` for explicit color control.
- Support `--no-progress` for logs and CI.

This follows CLIG, NO_COLOR, and AWS CLI output-format practice.

## Naming

Prefer names users naturally reach for:

- `status`: short current-state summary
- `check`: diagnostic checks, optionally scoped by domain
- `diagnose`: classify existing error text
- `bundle`: collect support context
- `up`: prepare the project
- `shell`: enter runtime
- `run`: run one command in runtime

Avoid overlapping verbs for the same job. If `check` becomes the diagnostic surface, do not keep a separate `doctor` surface with different defaults unless there is a strong product reason.

## Output Vocabulary

Use a small shared vocabulary for structured reports and verbose diagnostics:

- `project`: what runtime is being inspected
- `ready`: things that are ready
- `attention`: things to review or fix
- `skipped`: checks intentionally not run
- `next`: copyable next commands
- `status`: final ok/error and counts

Prefer concrete evidence:

```text
found: CUDA 12.2
need: CUDA 12.4
run: robo check cuda --deep
```

Avoid generic advice:

```text
something is wrong with CUDA
try reinstalling drivers
```

## Documentation Shape

Use Diataxis for docs structure:

- tutorials: first project and first robotics runtime
- how-to guides: install, add graphics, add CUDA, debug OpenGL, debug native build
- reference: commands, `robo.nix`, components, JSON schemas
- explanation: why robo exists, runtime boundary, uv boundary, Nix boundary

The failure guide should eventually be generated from `data/failures/*.toml`. Until then, keep it aligned manually and treat the data model as the future source of truth.

## Iteration Model

Do not implement UX changes as large internal phases, and do not treat the slices below as a fixed waterfall roadmap. Ship one thin user-visible workflow, validate it on real projects, revise the backlog, then choose the next highest-value slice.

This follows the practical lesson from mature tools: Pixi, uv, Homebrew, conda, and kubectl earn trust through small commands that work every day, not through one large diagnostics framework landing all at once.

The loop is:

```text
pick one user problem
ship the smallest useful workflow
test it on real projects
update the design from what happened
choose the next slice from evidence
```

The current first hypothesis is:

```text
If `robo check graphics` and `robo check native` can explain real dexmate-teleop failures clearly,
then the check/diagnose/status split is worth building on.
```

The next slice should be chosen after this hypothesis is tested. Do not implement later slices just because they are listed here.

### Candidate Slice: Useful Checks

Ship a minimal `robo check` that is immediately useful.

- `robo check`
- `robo check graphics`
- `robo check native`
- clean default human output
- `--verbose` evidence output
- redirected output without spinner frames
- snapshots for healthy, blocked, irrelevant, and uncertain checks

This slice proves the core UX: `check` is where users go when the runtime feels broken.

Do not wait for a full failure knowledge base. Hardcode only the minimum reusable probes needed to validate the output model, then move expanding coverage into data once the shape feels right.

### Candidate Slice: Minimal Diagnosis

Add `robo diagnose [FILE|-]` with a tiny set of high-value failure entries:

- Python/native ABI mismatch
- missing Qt6 CMake package
- missing `libGL.so.1`
- missing `libcuda.so.1`
- stale or externally-created `.venv`

Ship:

- human output with confidence-aware wording
- `robo.diagnostic.v1` JSON
- matcher tests for high, medium, low, and negative-pattern confidence
- bounded log input
- docs for the included failure entries

This slice proves that `diagnose` is different from `check`: it explains existing error text instead of probing the current machine.

### Candidate Slice: Support Bundle

Add a narrow `robo bundle`:

- local bundle directory under `.robo-nix/bundles/`
- small `summary.md`
- `diagnostics.json` when a diagnosis exists
- `runtime-contract.json`
- `host-facts.json`
- `robo.bundle.v1` manifest JSON
- default redaction for common secret names
- redaction tests

This slice should not collect everything. It should collect enough context for a maintainer or agent to avoid the first three rounds of "what OS, what Nix, what runtime, what did robo see?"

### Candidate Slice: First-Run Polish

Refine the main day-one workflow after the first diagnostic slices have real output:

- `robo up`
- `robo up --shell`
- `robo status`
- `robo shell`
- progress labels
- first-run docs
- no implicit `uv sync`
- no "run uv sync now?" prompt

This slice proves that the happy path feels better, not just that the failure path is informative.

### Candidate Slice: Coverage Growth

Grow coverage only after the core loops feel right:

- more `robo check <domain>` coverage
- data-driven `robo check <tool>` entries
- more failure entries
- generated failure guide from `data/failures/*.toml`
- optional fuzzy search over known failures
- explicit bootstrap configuration replacing legacy script probing
- `robo add` only if runtime component editing is ready as a product surface

This slice should mostly be metadata and docs. If adding support for a common tool requires large Rust rewiring, the extension model is still too rigid.

### Iteration Rules

Every iteration must be:

- releasable
- documented
- covered by focused tests
- honest about unsupported cases
- small enough to revert or revise
- useful without assuming the later slices exist

Avoid "framework first" work. If a candidate slice does not improve a real user workflow, postpone it. If real usage disproves the current design, update the design before adding more commands.

## Acceptance Tests

Before calling a UX iteration implemented, tests should cover:

- human output snapshots for `up`, `status`, `check`, `diagnose`, and `bundle`
- redirected output has no spinner frames
- `NO_COLOR=1` removes ANSI color
- `--json` output is valid JSON and has no human labels
- `--verbose` includes evidence without changing the default output shape
- weak diagnostic matches do not print direct fixes
- negative patterns lower confidence
- huge logs are bounded
- bundle redaction catches common token and credential names
- `robo check graphics`, `cuda`, and `native` fail gracefully on hosts without those capabilities
- docs examples match the implemented command names

## Migration Notes

Before alpha, backwards compatibility is not required for undocumented behavior. Prefer the clean command model over aliases that keep old ambiguity alive.

The current bootstrap script probing should be treated as legacy design debt. Replace repo-specific script discovery with explicit bootstrap configuration plus generic runtime-failure classification.

If removing a command, update:

- CLI help
- user workflow docs
- diagnostics docs
- shell integration docs
- captured-output tests
