# Agent Handoff

Behavioral guidelines in this file bias toward caution over speed. For trivial tasks, use judgment, but keep diffs small and easy to audit.

## Current Direction

`robo-nix` is a robotics environment toolkit powered by Nix and uv.

The product should optimize for people who want to work on robot learning, not learn Nix first.

The hard boundary is:

- `uv` owns Python versions, virtual environments, Python packages, and `uv.lock`.
- Nix owns runtime dependencies, native libraries, CUDA/graphics/ROS/simulator tooling, compilers, and shell environment.
- The Rust `robo` CLI should own user-facing workflow, diagnostics, and command wrapping.
- Runtime inference coverage should live in data/Nix metadata, not compiled Rust logic.

Do not make Nix-managed Python a first-class product mode unless real users prove that need. Do not build a central Python package registry or preset matrix.

Do not make `robo` run `uv sync` implicitly or offer an interactive "run uv sync now?" prompt. `robo` exists to make the uv environment work by providing the native/runtime layer and diagnostics; uv syncing remains an explicit user or project command because dependency groups, extras, package indexes, editable sources, and install policy are project-owned.

Python interpreter selection must prioritize robotics compatibility and cache availability, not only whatever the current nixpkgs happens to expose. `cachix/nixpkgs-python` is the intended interpreter source for broad Python-version coverage, including older versions such as Python 3.11 that may fall out of current nixpkgs. Do not “optimize” `python-uv` to prefer nixpkgs for minor versions just because it evaluates locally; that regresses downstream users when nixpkgs drops older interpreters. If CPython starts compiling, fix substituter/cache wiring (`nixpkgs-python.cachix.org` plus `--accept-flake-config`) before changing interpreter selection.

`robo-nix` is not here to adapt to every downstream project's environment policy. It should establish a consistent standard that downstream projects can converge on. Until they do, `robo` should print clear expectations, observed facts, and debugging context, not auto-solve project-specific dependency group choices, optional extras, source pins, or workflow conventions with narrow heuristics.

When a failure comes from a project-owned layer such as `pyproject.toml`, `uv.lock`, dependency groups, optional extras, editable sources, Git/LFS pins, or package indexes, keep `robo`'s response generic and reusable:

- explain which layer owns the failing contract
- say what `robo` expected and what is not expected
- pass through enough underlying tool output to debug
- avoid inferring or enabling project-specific uv groups, extras, pins, or install modes
- add reusable diagnostics only when they describe a product boundary or common failure class

## CLI And Extensibility Boundary

Prefer Rust for product UX and generic mechanics:

- argument parsing, help text, colors, errors, hints, and command wrapping
- reading manifests and project files
- applying data-driven inference rules
- writing generated `flake.nix`, `robo.nix`, `.python-version`, and bootstrap wiring
- running Nix/uv subprocesses with plain-language diagnostics

Prefer Nix/data files for expandable product coverage:

- component and profile metadata
- runtime inference rules in [nix/metadata/runtime-inference.nix](./nix/metadata/runtime-inference.nix:1)
- package-to-component mappings
- workspace directory and bootstrap script discovery rules
- default profile selection
- text markers, path roots, and component suggestions used by `robo init`

Adding support for more Python packages, workspace shapes, script prefixes, or common runtime hints should usually be a data change, not a Rust rebuild. Rust should stay boring and generic: parse the manifest, scan the project, apply rules, explain what happened, and fail clearly when metadata is invalid.

Avoid user-facing shell scripts as product surfaces. Shell is acceptable for generated runtime snippets and downstream project-owned bootstrap scripts, but CLI behavior should not depend on maintaining parallel shell UX, color, parsing, or error handling.

Keep `robo init` deliberately boring:

- [crates/robo-cli/src/init/pipeline.rs](./crates/robo-cli/src/init/pipeline.rs:1) is a flat sequence, not a framework.
- [crates/robo-cli/src/init/probe.rs](./crates/robo-cli/src/init/probe.rs:1) should only collect observed facts into `ProbeResult`.
- [crates/robo-cli/src/init/spec.rs](./crates/robo-cli/src/init/spec.rs:1) is where observed facts are applied to generated config.
- Do not add traits, registries, plugin layers, event buses, or generic builders for init flow.
- If adding a new inference, prefer one manifest rule plus one `ProbeResult` field/vector and one focused test.

CLI output consistency is part of the product contract:

- Follow the CLI output design in [docs/developers/cli-ux.md](./docs/developers/cli-ux.md:1).
- Do not print user-facing status lines with raw `println!("g; s: ...")`, `println!("robo: ...")`, or similar ad hoc prefixes.
- Route human CLI output through the existing themed label helpers, or add a small local wrapper that calls those helpers.
- Keep captured output stable and grep-friendly while coloring only labels/prefixes in terminals.
- When adding a new command surface, verify both theming and captured-output tests. JSON output must stay raw machine-readable JSON with no labels or colors.
- Reuse the command's established wording, punctuation, and prefix style before adding new status vocabulary.

## Engineering Style

Default to simple, flat code.

- Prefer readable inline blocks over new config objects, state objects, or tiny helpers.
- Do not extract single-use abstractions just to organize code.
- Prefer improving readability in place over adding another layer of indirection.
- Prefer data-driven extension points over hardcoded decision tables when future coverage is expected to grow.
- When fixing bugs or adding features, first look for a smaller change: delete duplication, collapse state, or tighten an existing boundary before adding helpers, flags, threads, status fields, or debug plumbing.
- Do not pass arguments that are already the callee default unless spelling them out materially improves clarity.
- For long constructors, derivation attributes, generated command builders, and return-value builders, group fields by concern with blank lines and short natural comments when useful.
- Match existing style, even if you would choose a different style in a new project.
- Keep production-grade robustness, but do not add defensive handling for impossible scenarios.

Host runtime path handling:

- Do not treat hardcoded host path inventories as a scalable abstraction. Moving lists such as NVIDIA driver library search paths into shared metadata or common helpers is not automatically cleaner; it can centralize brittle host assumptions and make them harder to audit.
- Do not add generated-shell scans over host NVIDIA, Vulkan, EGL, WSL, distro, or driver-version directories. Existing host-driver visibility should be diagnosed from observed environment/tool output, not guessed by mutating `LD_LIBRARY_PATH`, `VK_ICD_FILENAMES`, `__EGL_VENDOR_LIBRARY_FILENAMES`, or package-specific variables.
- Do not add package-specific environment variables, path probes, or compatibility workarounds to generic runtime modules just because one downstream Python package fails. Prefer narrow downstream workarounds while debugging, then promote only a documented, product-level contract.
- For host GPU/driver discovery, prefer explicit diagnostics that report observed facts and ownership boundaries over expanding generated shell behavior. If a generic exported fact is needed, first design the contract, document it, and add focused validation; do not introduce it opportunistically during a playground bring-up.
- Keep host-driver path logic local to the component or CLI diagnostic that actually owns the behavior. Avoid duplicating or “deduplicating” path lists unless the new shape demonstrably reduces product surface area and has tests.

Comment style:

- Add comments only where behavior is easy to misunderstand or future iteration would otherwise require rediscovery.
- Keep comments casual and specific, such as `# runtime vars`, not formal section headers.
- Use attention markers when useful: `TODO`, `FIXME`, `NOTE`, `WARN`, or `BUG`.
- Add a marker when a feature is incomplete, context is missing, behavior depends on user discretion, or the implementation makes meaningful assumptions.
- Do not add a marker if the block already has one.
- Avoid defensive fallbacks for known project interfaces. Prefer direct access and fast failure; if an assumption is intentionally unresolved, leave a short marker comment instead of silently accommodating unprepared cases.

## Execution Rules

Think before coding:

- State assumptions when they matter.
- If multiple interpretations are plausible, surface them instead of silently choosing.
- Push back when a simpler or safer approach is clearly better.
- If something is unclear and a wrong assumption would create churn, ask.

Keep changes surgical:

- Touch only what the task requires.
- Do not refactor adjacent code just because it is nearby.
- Remove imports, variables, functions, or docs that your own changes made unused.
- Mention unrelated dead code or cleanup opportunities instead of deleting them.
- Every changed line should trace to the request or to verification needed by the request.
- Keep docs deduplicated. Prefer updating the canonical page for a topic over adding a second guide with overlapping policy.

Use goal-driven verification:

- Turn tasks into concrete success criteria before editing.
- For concrete bugs, reproduce the failing workflow first when practical, fix the blocking boundary, then rerun the reproducer before broader checks.
- For new features or cleanups, a failing test first is not required; make the smallest coherent change and run focused checks afterward.
- For multi-step tasks, use a brief plan with verification points when the work is nontrivial.

Keep edit loops cheap:

- Classify the failing layer before coding: uv/Python lockfile, generated project files, Nix runtime/native libraries, Rust CLI diagnostics, host GPU/driver, or downstream playground script.
- Run the narrowest useful check for that layer first, such as one filtered Rust test, `nix-instantiate --parse`, `bash -n`, or `py_compile`.
- Treat Isaac, datagen, GPU validation, full repository checks, and broad compilation as integration checks. Run them only after cheaper preflight checks say the host and runtime are capable of passing.
- Keep generated project files out of manual edit loops. Change source metadata or init logic, then let `robo init . --force` regenerate `flake.nix`, `robo.nix`, and bootstrap wiring.
- In nested playgrounds, check top-level and playground status separately with `git status --short` and `git -C <playground> status --short` before summarizing diffs.
- When a build hangs or repeated attempts diverge, write the working facts, failed attempts, and next narrow reproducer into a gitignored note under `.failure-modes/` before trying another broad variant.
- Before debugging a repeated-looking issue, read relevant notes under `.failure-modes/`, then summarize durable lessons back into `AGENTS.md` so future agents do not rediscover the same failure mode.
- For VitePress/Nix docs work specifically, check `.failure-modes/vitepress-nix.md` first. The root cause of the earlier hang was VitePress's interactive progress renderer spinning inside the Nix builder; the Nix build must set `CI=1`.
- For vague `robo up --shell` setup stalls, check `.failure-modes/up-shell-vague-runtime-status.md` first. Split the report by visible phase before changing behavior: runtime files, project bootstrap, CUDA compatibility, Nix shell evaluation, shell env capture, or shell launch.
- For `robo up --shell` stalls around `shell: applying runtime exports`, check `.failure-modes/up-shell-hidden-sync-prompt.md` first. The previous root cause was an interactive `uv sync` prompt hidden behind an active spinner, not slow export materialization.
- For init or test slowness around `nix flake lock --update-input robo-nix`, check `.failure-modes/flake-lock-refresh.md` first. Local `path:` and `git+file:` robo-nix sources should skip eager lock refresh; non-local refresh should be bounded and reported as skipped on network/cache failure.

When the user corrects an agent:

- Treat the correction as a signal to re-check assumptions, not as something to defend against.
- Verify the correction against local code, docs, tests, or primary sources when practical; users can be right, partially right, or mistaken.
- If the correction is right and reveals a reusable lesson, write the durable takeaway into `AGENTS.md` or a focused `.failure-modes/` note before continuing.
- If the correction is mistaken, explain the evidence briefly and keep the product goal in view.
- Do not repeat a corrected mistake after the takeaway has been recorded.

## Product North Star

The intended beginner experience is:

```bash
robo up robot-learning --yes
cd robot-learning
robo up --shell
uv sync
```

The installed workflow should keep Nix in the background:

```bash
robo up
robo check
robo shell
robo run <command>
```

## High-Priority TODO

1. Grow the Rust `robo` CLI into the primary UX.
   It should wrap Nix commands, hide `--extra-experimental-features`, detect missing Nix/flakes support, and print plain-language fixes.

2. Make `check` the main diagnostics surface.
   It should validate host prerequisites, Nix/flakes availability, workspace layout, supported platform, uv state, native runtime libraries, GPU/CUDA expectations, and likely missing runtime dependencies. Keep `diagnose` focused on classifying existing error logs.

3. Keep Python ownership in uv.
   Generated projects should use `.python-version`, `pyproject.toml`, and `uv.lock`. Nix should provide `uv` and the native/runtime layer that uv-installed packages need.

4. Improve native/runtime diagnostics.
   Catch and explain common robotics failures such as missing `libstdc++.so.6`, `libGL.so.1`, FFmpeg libraries, CUDA driver/runtime mismatch, and native extension build failures.

5. Tighten graphics support without overstating it.
   Current `x11-gl` handling bridges detected host NVIDIA GLVND libraries for desktop OpenGL, but AMD/Intel edge cases, PRIME/offload policy, Wayland-specific failures, headless/remote rendering modes, and a nixGL-style one-command launcher are not solved product surfaces yet. Keep these gaps explicit in docs and diagnostics. When expanding graphics support, prefer learning from nixGL's provider detection and command-wrapper model over mutating the whole development shell.

6. Keep templates non-product until explicit maintainer approval.
   Use `robo up` / `robo init` as the onboarding path. Placeholder template files may exist to define layout, but do not expose them as a public workflow until real usage proves them.

7. Keep docs split by audience.
   User docs live under [docs/users](./docs/users/getting-started.md:1) and should assume zero Nix background. Developer docs live under [docs/developers](./docs/developers/overview.md:1) and can explain flakes, components, metadata, and maintainer workflows.

8. Keep verification strict for AI-assisted changes.
   Follow the AI-usage contract in [CONTRIBUTING.md](./CONTRIBUTING.md:1).

## Existing Verification Workflow

Before closing substantial work, run:

```bash
nix run .#repo-fmt
nix run .#repo-lint
nix run .#repo-profile
bash tests/regression-api.sh
bash tests/profile-validation.sh
bash tests/fixture-validation.sh
bash tests/robo-init-validation.sh
bash tests/gpu-validation.sh
nix flake check
```

`tests/gpu-validation.sh` requires a suitable NVIDIA host.

## Notes

- The scalable extension mechanism is `robo-nix.lib.mkProjectFlake`, not central preset expansion.
- Do not preserve backward compatibility before a behavior is specified in docs. Pre-spec compatibility creates hidden surface area.
- Templates are currently withheld pending manual approval. Do not publish one casually.
- Generated projects should point at the packaged `robo-nix` source by default. Do not rely on `/usr/share`, GitHub queries, or an ambient local checkout for installed CLI behavior.
- Repo-root `Cargo.toml` / `Cargo.lock` are intentional because Rust is the main CLI workspace. Docs-only Node tooling belongs under `docs/package.json` and `docs/package-lock.json`. Do not reintroduce root `package.json`, root `pyproject.toml`, or root `.python-version` unless the repo gains a real root Node or Python product surface.
- The installer lives at [scripts/install.sh](./scripts/install.sh:1); keep README and docs curl commands pointed at `develop/scripts/install.sh` for now.
- TODO: before the public release branch moves to `master`, update README and docs curl commands from `develop/scripts/install.sh` to `master/scripts/install.sh`.
- Project-specific robot/source policy should stay in downstream projects unless it becomes broadly reusable.
- The product north star is filling the native runtime gap implied by `pyproject.toml` and `uv.lock`.
- Runtime inference rules live in [nix/metadata/runtime-inference.nix](./nix/metadata/runtime-inference.nix:1); known failure modes are documented in [docs/users/diagnostics.md](./docs/users/diagnostics.md:1).
- Keep names user-facing and natural. `robo` is the CLI name; avoid reintroducing `rob` or `project-init` as public surfaces.
- Keep tests fast for development. Prefer the focused edit-loop checks in `tests/dev-check.sh`, and reserve full validation for broader changes or CI.
- Recent local profiling baseline on this host was roughly:
  - default app eval: `2.41s`
  - `nix flake show --all-systems`: `4.79s`
