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
- runtime inference rules in [nix/modules/runtime-inference.nix](./nix/modules/runtime-inference.nix:1)
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

## Product North Star

The intended beginner experience is:

```bash
robo init robot-learning
cd robot-learning
robo check
robo sync
robo shell
```

The current alpha exposes `robo` through Nix while distribution is still being designed:

```bash
nix run github:ausbxuse/robo-nix#robo -- init .
nix run .#default -- --check
nix develop
uv sync
```

## High-Priority TODO

1. Grow the Rust `robo` CLI into the primary UX.
   It should wrap Nix commands, hide `--extra-experimental-features`, detect missing Nix/flakes support, and print plain-language fixes.

2. Make `check` the main product surface.
   It should validate host prerequisites, Nix/flakes availability, workspace layout, supported platform, uv state, native runtime libraries, GPU/CUDA expectations, and likely missing runtime dependencies.

3. Keep Python ownership in uv.
   Generated projects should use `.python-version`, `pyproject.toml`, and `uv.lock`. Nix should provide `uv` and the native/runtime layer that uv-installed packages need.

4. Improve native/runtime diagnostics.
   Catch and explain common robotics failures such as missing `libstdc++.so.6`, `libGL.so.1`, FFmpeg libraries, CUDA driver/runtime mismatch, and native extension build failures.

5. Keep templates non-product until explicit maintainer approval.
   Use `robo init` as the alpha onboarding path. Placeholder template files may exist to define layout, but do not expose them as a public workflow until real usage proves them.

6. Split docs into beginner and advanced paths.
   Beginner docs should assume zero Nix background. Advanced docs can explain flakes, components, and maintainer workflows.

7. Keep verification strict for AI-assisted changes.
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
- Project-specific robot/source policy should stay in downstream projects unless it becomes broadly reusable.
- The product north star is filling the native runtime gap implied by `pyproject.toml` and `uv.lock`.
- Runtime inference rules live in [nix/modules/runtime-inference.nix](./nix/modules/runtime-inference.nix:1); known failure modes are documented in [docs/diagnostics.md](./docs/diagnostics.md:1).
- Keep names user-facing and natural. `robo` is the CLI name; avoid reintroducing `rob` or `project-init` as public surfaces.
- Keep tests fast for development. Prefer the focused edit-loop checks in `tests/dev-check.sh`, and reserve full validation for broader changes or CI.
- Recent local profiling baseline on this host was roughly:
  - default app eval: `2.41s`
  - `nix flake show --all-systems`: `4.79s`
