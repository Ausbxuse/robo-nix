# Developer Overview

Keep `robo-nix` narrow: it prepares the native runtime layer for uv-managed
robot-learning projects. It should not grow into a general environment manager
or Python package resolver.

## Runtime Flow

`robo shell` and `robo run [--] <command>` share the same preparation path:

1. Read `.python-version`. Missing or empty files are hard errors.
2. Create `.robo-nix/` if needed.
3. Create `flake.nix` only when missing. Existing non-robo flakes are not
   overwritten.
4. Create `robo.nix` only when missing.
5. During first bootstrap in Git worktrees, ensure `.robo-nix/` is listed in the
   workspace `.gitignore` without touching the Git index.
6. Evaluate the Nix runtime shell with
   `nix develop --accept-flake-config --command sh -c 'printf ...; env -0'`.
7. Hide successful Nix stdout/stderr, parse the NUL-separated environment,
   clear the inherited host environment, and apply the captured environment to
   the final process.
8. For `robo shell`, launch the selected interactive shell directly with the
   resolved environment.
9. For `robo run`, strip one optional leading `--` after `run`, then launch the
   requested command directly with the resolved environment.
10. Propagate the final shell or command exit status. A nonzero user command is
   not treated as a robo setup failure.

The preparation code lives in `src/bootstrap.rs`. Nix environment capture lives
in `src/nix_env.rs`. The command wiring lives in `src/main.rs`.

## Ownership

- uv owns `pyproject.toml`, dependency groups, extras, lockfiles, virtualenv
  sync, and Python package resolution.
- Nix owns CPython, native tools, runtime libraries, shell environment, and
  component implementation.
- Rust owns command UX, diagnostics, project bootstrap, template rendering,
  shell launch, and command wrapping.
- `robo.nix` is canonical after first creation. `robo shell` and active shell
  refresh must not rewrite it.

## Generated Files

Generated text lives in checked-in resource files and is embedded with
`include_str!`:

- `src/templates/project/flake.nix`
- `src/templates/project/robo.nix`
- `src/templates/shell/*`
- `src/metadata/runtime-inference.tsv`

The generated project `flake.nix` should stay minimal: cache hints, one
`robo-nix` input, and a handoff to
`robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix`.

Robo-owned Nix commands pass the public robo-nix cache substituters and trusted
keys directly. The generated `nixConfig` stays as portable project metadata, but
runtime setup should not depend on the host system substituter list.
Before `nix develop`, runtime setup prefetches dev-shell input outputs with
local builds disabled; this lets cached outputs be copied while leaving normal
Nix evaluation to handle local shell derivations and uncached project state.

Generated project flakes use `github:ausbxuse/robo-nix/master` as the default
`robo-nix` input, including when `robo` was installed from a local Nix profile.
This keeps newly bootstrapped projects portable and avoids locking generated
flake state to a local checkout or Nix store source.

The packaged source is explicitly filtered before it is copied to the Nix store.
Repo-local caches and generated trees such as `.robo-nix/`, `target/`,
`docs/node_modules/`, and VitePress cache/dist outputs are excluded.

`ROBO_NIX_DEFAULT_SOURCE_URL` can override the generated flake input URL for
focused local-source tests.

## Runtime Inference

Runtime inference is first-bootstrap only. If `robo.nix` already exists,
inference is skipped.

Rules live in `src/metadata/runtime-inference.tsv`, not hardcoded Rust
conditionals. Current known components are:

- `python-uv`: CPython from `nixpkgs-python` plus `uv`, including the CPython
  shared library path for packages that embed the interpreter.
- `native-build`: compiler tools, a generic libc development path, plus runtime
  `libstdc++`, zlib, and legacy `libcrypt`.
- `linux-headers`: Linux kernel headers for native input packages such as
  `evdev`.
- `desktop-gl`: desktop graphics client libraries, Vulkan loader, GLFW
  windowing, GLU, and legacy X libraries used by simulator stacks.
- `qt6`: Qt6 base and Core5Compat build/runtime support for Qt CMake projects,
  services, and viewers.
- `cuda-toolkit`: Nix-owned CUDA compiler, headers, and CUDA runtime build
  surface.

Inference reads `[project].dependencies`, `[project].optional-dependencies`,
`[dependency-groups]`, and legacy `[tool.uv].dev-dependencies` arrays from
`pyproject.toml`, normalizes package names, and adds matching components. It
also reads local `[tool.uv.sources]` path dependencies and follows their
`pyproject.toml` metadata when available, including extras selected by the root
requirement such as `local-package[full]`.
When an existing `uv.lock` is present, inference also reads resolved package
names from the lockfile as static evidence for transitive runtime needs.

This is a static metadata walk, not Python package solving. Remote package
metadata is left to uv, and first bootstrap prints attention diagnostics when a
local source cannot be inspected or when remote package metadata was skipped.
Missing or invalid root `pyproject.toml` produces a base runtime instead of
failing.

## Project Nix Library

The reusable project shell implementation is in `src/nix/project-flake.nix`.
It reads `.python-version`, imports the project `robo.nix`, validates component
names, and constructs the `devShell`.

Important shell behavior:

- `UV_PYTHON` points at the Nix-managed CPython.
- `UV_PYTHON_DOWNLOADS=never` prevents uv from downloading another Python.
- `UV_PROJECT_ENVIRONMENT` defaults to `$PWD/.venv`.
- The `python-uv` component wraps `uv pip install` so ad hoc installs target
  `$UV_PROJECT_ENVIRONMENT/bin/python` when that venv exists and no explicit
  uv target was provided.
- Python activation scripts may add their virtualenv marker. The prompt hook
  keeps the `[robo]` marker single when activation rewrites `PS1` or `PROMPT`.
- `UV_CACHE_DIR` defaults to `$PWD/.robo-nix/uv-cache`.
- `PYTHONHOME` and `PYTHONPATH` are unset.
- `LD_LIBRARY_PATH` is built from selected component runtime libraries plus
  `extraRuntimeLibraries`.
- `python-uv` contributes the CPython `lib/` directory so native packages can
  load `libpython` by soname.
- `native-build` exports `ROBO_NIX_LIBC_DEV` for scripts that need to inspect
  the compiler libc development prefix.
- `linux-headers` exports `ROBO_NIX_LINUX_HEADERS`, `CPATH`, and
  `C_INCLUDE_PATH`.
- `cuda-toolkit` exports CUDA build variables.
- `native-build` exposes CMake through a diagnostic wrapper. It preserves CMake
  behavior and may print a generic hint when `find_package` cannot locate a
  package config file, but it must not infer or inject package-specific
  `*_DIR` paths.

Host CUDA drivers remain host-owned. The Rust launch path may add a narrow
`libcuda.so.1` bridge when project dependencies or `uv.lock` indicate CUDA
wheels that need the host driver. The bridge honors `ROBO_NIX_LIBCUDA_PATH`,
supports `ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1`, and does not add host EGL/Vulkan
graphics policy.

Host graphics wrapper policy is Nix-owned. The generated shell defaults to
`hostGraphics = "auto";`, uses `/run/opengl-driver` on NixOS hosts, and uses
the generic robo-provided nixGL wrapper on other Linux hosts. `hostGraphics =
"nixgl-nvidia";` requires the NVIDIA nixGL wrapper and may use
`ROBO_NIX_NVIDIA_VERSION` when host driver version detection is unavailable.
Robo imports graphics variables from nixGL without adding its own PRIME
render-offload defaults. Rust must not maintain host GLX/EGL/GBM graphics
wrapping.

## Search

`robo search <library>` is lookup-only. It first tries local `nix-locate`, then
falls back to the prebuilt `nix-index-database` flake. It prints candidate
`pkgs.*` attributes and an `extraRuntimeLibraries` snippet. It must not mutate
`robo.nix` or become a package resolver.

## Active Shell Refresh

Interactive shells receive startup files under `.robo-nix/shell-startup/`.
Those files prefix the user's existing prompt with `[robo]` and call the hidden
`robo __shell-refresh <shell>` helper at prompt time.

Refresh fingerprints these runtime inputs:

- `flake.nix`
- `flake.lock`
- `.python-version`
- `pyproject.toml`
- `uv.lock`
- `robo.nix`
- `.venv/bin/python`

When the fingerprint changes, refresh runs the same Nix environment capture,
exports the refreshed environment into the current shell, and updates the active
fingerprint state. It reports changed paths. It does not run `uv sync`, migrate
the shell process, or rewrite `robo.nix`.

`robo shell` and `robo run` cache the captured Nix runtime environment under
`.robo-nix/` by the same runtime input key. Cache hits skip `nix develop` after
verifying referenced `/nix/store` paths still exist. Active shell fingerprints
are computed from the final launched environment so prompt refresh does not
immediately re-run after host CUDA or library path preparation.

`robo refresh` removes robo-owned `.robo-nix/` state. In an active runtime
shell, it writes a manual refresh request that is part of the runtime input key;
the prompt hook consumes that request after a successful environment refresh.

## Changelog

Each changelog-backed change should:

- Start from the review ledger.
- Call out conflicts before coding.
- Keep diffs narrow.
- Update `AGENTS.md` only for durable rules.
- Add focused verification notes to `docs/changelog/YYYY-MM-DD-short-title.md`.

The `docs/changelog/` ledger is intentionally excluded from the public
VitePress build. Keep durable user and developer guidance in the user/developer
pages instead of relying on internal changelog history.

## Verification

Use the narrowest useful checks first:

```bash
cargo test
nix-instantiate --parse flake.nix
npm --prefix docs run build
```

When generated project files change, also render a temporary project and parse
its generated `flake.nix` and `robo.nix`.

## Docs Deployment

The documentation site is built by `.github/workflows/docs.yml` and deployed to
GitHub Pages from `master` through GitHub Actions. Repository Pages settings
should use **GitHub Actions** as the build source, and the `github-pages`
environment must allow deployments from `master`.
