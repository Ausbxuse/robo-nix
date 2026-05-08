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
