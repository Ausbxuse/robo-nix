# Repository Workflow

This page covers repo-level maintenance.

## Documentation

The VitePress site is the maintained documentation surface:

```bash
nix build .#docs
nix run .#docs-serve
```

Keep docs organized by audience:

- `docs/users/` for usage, setup, workflows, runtime expectations, and troubleshooting
- `docs/developers/` for architecture, CLI contracts, metadata design, and maintainer workflow

Do not reintroduce a second maintained Rustdoc-style documentation surface unless there is a concrete release need.

The docs site owns its Node toolchain under `docs/package.json` and `docs/package-lock.json`. Keep root `package.json` out of the repo unless the root gains a real Node product surface.

## Formatting and Linting

Run:

```bash
nix run .#repo-fmt -- --check
nix run .#repo-lint
```

Use `nix run .#repo-fmt` only when you intend to apply formatting.

## Tests

Focused CLI tests:

```bash
cargo test -p robo-cli
```

Regression and fixture checks:

```bash
bash tests/regression-api.sh
bash tests/profile-validation.sh
bash tests/fixture-validation.sh
bash tests/robo-init-validation.sh
```

Full flake validation:

```bash
nix flake check
```

GPU validation is host-dependent:

```bash
bash tests/gpu-validation.sh
```

Do not treat GPU, Isaac, or broad Nix checks as cheap inner-loop tests.

## Failure Modes

When a build or runtime workflow appears to hang, record the facts in `.failure-modes/` before trying many broad variants.

Known notes:

- `.failure-modes/vitepress-nix.md`: VitePress needed `CI=1` in the Nix build.
- `.failure-modes/up-shell-hidden-sync-prompt.md`: `robo up --shell` appeared stuck because a spinner hid an interactive sync prompt.

The directory is gitignored. Read relevant notes before debugging repeated-looking failures, and summarize reusable takeaways in `AGENTS.md` when they should affect future agents.
