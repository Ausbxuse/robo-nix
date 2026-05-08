# Iteration 001 - Minimal Core

## Goal

Start from an orphan branch and rebuild only the smallest user workflow that
`robo-nix` needs:

```bash
robo init robot-learning
cd robot-learning
robo shell
uv sync
```

## Scope

Implemented in this iteration:

- A Rust CLI package named `robo`.
- `robo init [path] [--force]`, which writes generated project files.
- `robo check`, which validates the generated file contract without running Nix
  or uv.
- `robo shell`, which delegates to `nix develop --accept-flake-config`.
- `robo run <command> [args...]`, which delegates to
  `nix develop --accept-flake-config --command ...`.

Explicitly not implemented:

- `robo diagnose`.
- Runtime inference.
- CUDA, graphics, ROS, simulator, or package-specific diagnostics.
- Implicit `uv sync`.
- Templates as a public product surface.

## Code Shape

`src/main.rs` is one file on purpose. The command set is still small enough that
splitting modules would add navigation cost without clarifying ownership.

The generated Nix project is standalone. It uses `cachix/nixpkgs-python` for
CPython and keeps that input following the same `nixpkgs` used for runtime
packages.

`robo check` only checks the file contract owned by `robo init`. It does not
infer project policy, inspect dependency groups, or decide whether `uv sync`
should run.

## Verification

Run for this iteration:

- `cargo check`
- `cargo test`
- `cargo run -- init /tmp/robo-minimal-smoke`
- `nix-instantiate --parse flake.nix` in the generated smoke project.
- `nix-instantiate --parse robo.nix` in the generated smoke project.
- `target/debug/robo check` in the generated smoke project.

Not run:

- `cargo fmt --check`, because this host does not have `cargo-fmt`.
- `rustfmt --check src/main.rs`, because this host does not have `rustfmt`.

## Review Ledger

Reviewer concerns raised after this iteration should be recorded here first.
They should not be implemented one by one during review. The next iteration
should group accepted concerns into the smallest coherent change.

Pending concerns:

- Maintain an effective root `AGENTS.md` as part of the rebuild. It should be
  refined during iterations when durable working rules emerge, not left as a
  stale copy of the old repository guidance.

Proposed handling:

- Add a root `AGENTS.md` in the next implementation iteration before growing
  product code further.
- Keep `AGENTS.md` short and operational: project north star, code ownership
  boundaries, iteration/review rules, verification expectations, and known
  failure-mode policy.
- Keep iteration-specific debate in `docs/development/iteration-*.md`; promote
  only durable lessons into `AGENTS.md`.
- Review `AGENTS.md` as part of each iteration close-out, and update it only
  when the change would prevent future churn or rediscovery.
- Reconsider embedded generated Nix strings in Rust. The current minimal CLI
  writes `flake.nix` and `robo.nix` from Rust string literals, which is
  tolerable for iteration 001 but likely not a clean long-term boundary.

Proposed handling:

- Keep Nix source as `.nix` template files once the generated project contract
  grows beyond a few lines.
- If Rust must ship templates, use `include_str!` over standalone template
  files so reviewers can read Nix as Nix and run Nix-specific checks on it.
- Keep Rust responsible for command parsing, file placement, overwrite policy,
  and substituting a small number of scalar values.
- Keep Nix responsible for shell structure, package lists, runtime library
  behavior, and any future data-driven runtime logic.
- Add focused tests that render templates into a temporary project and parse the
  rendered Nix with `nix-instantiate --parse`.
- Avoid building a Rust-side Nix AST or formatter unless `robo` becomes a
  general Nix expression generator, which is not the current product goal.

## Candidate Scope For Iteration 002

Requested by review:

- Separate heavy raw strings in general, not only generated `.nix` files.
- Add minimal runtime inference.
- Add flake-based toolchains.
- Add minimal error handling and diagnostics.
- Add minimum user and developer docs.

Proposed handling:

- Move substantial generated text into template/resource files before adding new
  behavior. This should cover Nix, Markdown/help text, generated TOML, and any
  future shell snippets. Rust should keep small inline strings only when they are
  ordinary messages or one-line generated values.
- Add a tiny template renderer with explicit placeholders, such as
  `{{project_name}}` and `{{python_version}}`. Do not add a general template
  engine until the project has enough variation to justify one.
- Keep runtime inference data-driven. For the first version, scan only
  `pyproject.toml` dependency names and map a small set of package markers to
  runtime components or notes. Put the rules in data owned by the runtime layer,
  and make Rust responsible only for loading rules, applying them, and explaining
  what matched.
- Keep inference advisory and auditable. It can select minimal generated
  components during `robo init` by modifying generated `robo.nix`, but it must
  report what matched and must not run `uv sync`, choose uv dependency groups, or
  infer optional extras.
- Add a root `flake.nix` for the rebuild branch itself. Assumption: "flake-based
  toolchains" means the repository should pin and expose the tools needed to
  build, test, format, and parse-check generated files, not only generate
  downstream project flakes.
- Keep the root flake small: one dev shell, one formatter/check path, and any
  narrow checks needed for the iteration. Avoid reintroducing the old repository
  support framework.
- Center diagnostics in `robo shell` for now instead of adding `robo check` or
  reintroducing `robo diagnose`: missing Nix, missing generated files, invalid
  template data, unsupported host system, and generated Nix parse failures.
- Keep diagnostics bounded: say the owning layer, expected file/tool, observed
  failure, and next command. When `robo shell` fails, write enough debug context
  for a GitHub issue or agent handoff without trying to debug downstream project
  policy.
- Add minimum docs by audience: a user getting-started page for the four-command
  workflow, a developer overview explaining the iteration process and code
  boundaries, and a short root `AGENTS.md`.
- Remove the iteration-001 `robo check` surface for now. Reintroduce it only if
  `robo shell` grows enough that a separate preflight command has a clear
  boundary.
- Remove the hardcoded Python default. `robo shell` should read the uv-style
  `.python-version` file and report a clear error when it is missing. Generated
  projects should make the Python version explicit in `.python-version` instead
  of baking a default into Rust.
- Add useful comments with markers such as `TODO`, `NOTE`, `DEBUG`, `FIXME`,
  `WARN`, and `BUG` where they clarify project boundaries, incomplete behavior,
  or future work. Keep comments trustworthy and specific, not decorative.

Open design questions before implementation:

- Should runtime inference rules live as Nix data, TOML/JSON data consumed by
  Rust, or both rendered into generated Nix?
- What is the cleanest rule data format for iteration 002 while keeping future
  migration to Nix-owned metadata easy?
- Should the root flake provide `cargo fmt` through `rustfmt`, even though this
  host currently lacks `rustfmt` outside Nix?
- Should generated Markdown docs be templates, or should docs stay handwritten
  and only generated project files move to templates?
- `include_str!` clarification: this means checked-in template files are compiled
  into the `robo` binary at build time. The alternative is installing template
  files beside the binary and reading them at runtime. The compiled-in approach
  keeps installation simple; runtime files are easier to patch locally but add
  packaging surface. Decision: use checked-in templates plus `include_str!` for
  iteration 002.
- Confirmed spelling: uv uses `.python-version` with a hyphen. The earlier
  `.python_version` mention was a typo.

Alignment updates from review:

- `robo-nix` is not beginner-only, but it should be the most beginner-friendly
  robot-learning runtime environment tool. Its narrow focus is robot learning,
  not general development environments like `devenv`.
- Highest priorities are robustness and ease of use.
- `robo diagnose` is out of scope and should be removed. Diagnostics belong
  inside scoped commands, especially `robo shell`, and should help developers or
  agents fix issues from pasted debug logs.
- Runtime inference should be data-driven and separate from Rust command logic.
- Generated `robo.nix` can be modified by inference, following the clean shape
  of the original repo's generated `robo.nix`.
- Generated downstream `flake.nix` should stay small and delegate editable policy
  to `robo.nix`.
- The repo should get a minimal root `flake.nix` for toolchains.
- User docs and developer docs should be separate from the beginning.
- Nix owns interpreter/native/runtime/toolchain layers. uv owns Python package
  resolution, virtualenvs, dependency groups/extras, and lockfiles.
- Iterations should ship fast, break fast, and iterate fast while preserving
  minimal completeness.

Consistency checks before implementation:

- Iteration 001 currently includes `robo check`; review says no `robo check` yet.
  Iteration 002 should remove it from the public surface.
- Iteration 001 currently hardcodes Python `3.11`; review says there should be
  no Rust default. Iteration 002 should read `.python-version` and fail clearly
  if absent.
- Earlier candidate wording mentioned `robo check` diagnostics. That is now
  superseded by the `robo shell`-centered diagnostic direction.
- `robo shell` may grow quickly if it owns all early diagnostics. The explicit
  review direction is to accept that for now and refactor only when it becomes
  unmanageable.

Confirmed consistency decisions:

- Remove `robo check`.
- Remove `robo init`.
- Read `.python-version`.
- Treat `robo shell` as the canonical command users primarily use.
- Use `.python-version`, not `.python_version`.
- Use checked-in template files compiled into the binary with `include_str!`.
- Keep `robo run`.
- If a command needs `.python-version` and it is missing, fail clearly instead of
  choosing a default Python version.
- `robo shell` should re-read project files and warn when generated runtime
  config looks stale relative to current project metadata.
- Start runtime inference with a tiny rule set: `torch`/`pytorch`,
  `opencv-python`, and `mujoco`.
- A small tab-separated rule file is acceptable in principle if it keeps the
  implementation dependency-free and easy to audit.
- `robo shell` should absorb the useful generated-file behavior from
  `robo init` through auto-detection/bootstrap, similar to the original repo's
  HEAD direction, while remaining explicit about what it writes.

Resolved clarification:

- `robo init` should be removed entirely. Iteration 002 should reshape the CLI
  around `robo shell` and `robo run`, with `robo shell` responsible for detecting
  the project state and preparing the generated runtime files it owns.

Remaining shell-bootstrap questions:

- When `flake.nix` or `robo.nix` is absent but `.python-version` exists, should
  `robo shell` write the missing generated files automatically, or should it
  print the intended diff/path and require an explicit flag?
- When generated files exist but are stale relative to `pyproject.toml` or
  `.python-version`, should `robo shell` warn only, repair with a flag, or repair
  automatically if the generated ownership marker matches?
- What ownership marker should generated files carry so `robo shell` can
  distinguish files it may repair from user-owned files it must not overwrite?

## Supervisor Check

Before iteration 002, confirm whether this is the right minimal product core:

- Is the absence of `robo diagnose` correct?
- Should `robo check` stay static for now, or should it run a bounded Nix
  preflight?
- Is Python 3.11 the right initial default for robot-learning compatibility?
- Should root `AGENTS.md` maintenance be a required close-out check for every
  iteration?
- Should iteration 002 move generated Nix into template files before any new
  runtime behavior is added?
- Are the candidate iteration-002 boundaries above correct, especially keeping
  inference advisory and keeping diagnostics inside `check`/`init` instead of
  bringing back `diagnose`?
- Confirm the consistency checks above before implementation begins.
