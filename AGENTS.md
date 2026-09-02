# Repository Guidance

`robo-nix` is a focused runtime environment tool for robot-learning projects.
It should be robust, easy to use, and narrow. Do not grow it into a general
environment manager.

## Current Product Shape

- `robo shell` is the canonical user command.
- `robo run [--] <command> [args...]` uses the same runtime preparation path as
  `robo shell`. It accepts one optional leading `--` after `run` as the standard
  end-of-options separator for commands that begin with `-`, and preserves later
  `--` arguments for the child command.
- `robo search <library>` is a lookup-only helper for missing native shared
  libraries.
- `robo refresh` clears robo-owned runtime state under `.robo-nix/`. Inside an
  active `robo shell`, it requests prompt-time environment refresh through the
  existing shell hook; outside a shell, it makes the next `robo shell` or
  `robo run` rebuild runtime cache state.
- In a downstream project, `robo update` updates the workspace `robo-nix`
  flake input, reinstalls the `robo` CLI binary from that updated input, and
  clears robo-owned runtime cache state so the next runtime command uses the
  updated lock. In the `robo-nix` source checkout, it reinstalls the binary
  from `.#robo` without expecting a child `robo-nix` lock node or updating the
  checkout's other flake inputs. It is not a general dependency updater.
- There is no `robo init`, `robo check`, or `robo diagnose` in the current
  product surface.
- `robo shell` may create missing runtime files during first bootstrap, then
  evaluates the Nix dev-shell environment and launches the user's shell with
  that environment.
- In a Git worktree, first bootstrap should ensure `.robo-nix/` is ignored by
  the workspace `.gitignore` without mutating the Git index or overwriting
  existing ignore rules.
- `.python-version` is required. `robo` does not choose a default Python
  version.
- `pyproject.toml` is owned by uv/project policy. `robo` must never create it.
- `robo.nix` is user-editable after first creation and is canonical for the
  shell.
- `robo shell` should launch the user's default interactive shell, not force
  Bash. Use `ROBO_NIX_SHELL` only as an explicit override.
- `robo shell` owns the visible `[robo]` prompt marker. Runtime shells should
  disable Python virtualenv activation prompt rewrites so copied
  `source .venv/bin/activate` commands do not duplicate prompt prefixes.
- Active `robo shell` sessions should refresh their runtime environment at the
  next prompt when runtime input files change. Refreshing may re-evaluate the
  Nix shell and export new variables, but it must not rewrite user-managed
  `robo.nix`.
- Shell environment caching must be keyed by the same runtime inputs used for
  refresh, and cache reuse must validate referenced Nix store paths. Active
  shell fingerprints should describe the final launched environment, not the
  parent process before runtime preparation.
- A successfully cached runtime environment must retain its referenced Nix
  store paths through profile-owned GC roots. If changed runtime inputs cannot
  be evaluated, `robo shell` and `robo run` should visibly fall back to the
  validated last working environment without re-keying it as current.
- Official CLI builds should carry source revision metadata. On the first
  `robo shell` or `robo run` for a newer CLI/older official project-lock pair,
  robo may best-effort update only the declared `robo-nix` input. Record the
  attempt so offline failure does not delay every command, never make that
  failure block runtime startup, and preserve prior runtime caches for normal
  last-working fallback.
- Use product language such as runtime environment, runtime shell, and runtime
  cache for robo-owned surfaces. Avoid naming robo concepts after generic dev
  environment tooling unless referring directly to Nix's dev shell primitive.
- `robo shell` must refuse to start from inside an active `robo shell`; nested
  shells make prompt hooks and refresh state harder to reason about.

## Ownership Boundaries

- uv owns Python package metadata, dependency groups, extras, virtualenv sync,
  and lockfiles.
- Nix owns the CPython interpreter, native tools, runtime libraries, and shell
  environment.
- `python-uv` must expose the CPython shared library path as runtime surface;
  Python packages may embed CPython or use `ctypes.CDLL("libpython...")`.
- Rust owns command UX, diagnostics, project-file preparation, and command
  wrapping.
- Runtime inference rules should live in data files, not hardcoded Rust
  conditionals.
- Runtime inference may read an existing `uv.lock` as static package evidence;
  this is not dependency resolution and must not fetch remote metadata.
- `robo search` may use `nix-locate`/nix-index data to suggest Nix packages,
  but it must not mutate `robo.nix`, become a Python package resolver, or grow a
  central package registry.
- Docs must describe the current product surface. Do not reintroduce `robo init`,
  `robo check`, or `robo diagnose` in user-facing docs until those commands are
  intentionally restored.
- Keep docs Node tooling under `docs/`; do not add root Node package files.
- Installer docs and scripts must end with the current `robo shell` workflow.
  Do not leave stale installer text pointing users at removed commands.

## Changelog Rules

- Keep each changelog-backed change small enough to review line by line.
- For concrete project failures with logs or a known command, reproduce the
  failure before coding whenever practical. Record the failing command and key
  error in the changelog entry before or alongside the fix.
- Record review concerns in `docs/changelog/YYYY-MM-DD-short-title.md` before
  turning them into code.
- Promote only durable operating rules into this file.
- Before starting an implementation pass, check for conflicts in the review
  ledger and call them out.

## Editing Rules

- Keep shipped templates, metadata, and hidden Nix implementation files under
  `src/`. Rust may embed resource files with `include_str!`.
- Avoid large raw generated strings in Rust.
- Use comments only when they clarify ownership, incomplete behavior, or a
  future hazard. Prefer specific markers such as `NOTE`, `TODO`, `FIXME`,
  `WARN`, `BUG`, and `DEBUG`.
- Do not commit local debugging artifacts into source, tests, or docs. Avoid
  local usernames, absolute home paths, project-specific names, and
  project-specific fixture paths; use generic temporary project names.
- When improving ergonomics, prefer transparent diagnostics over hidden
  compensation. `robo-nix` should make ownership boundaries obvious and provide
  actionable next steps, but it should not silently patch over missing
  project declarations, package metadata, or build-system handoffs with
  dependency-specific behavior.
- Do not overwrite a non-robo `flake.nix`.
- Generated project `flake.nix` should stay minimal and delegate runtime
  complexity to `robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix`.
- Do not rewrite an existing `robo.nix` from `robo shell`.
- Keep Nix-managed desktop graphics client libraries separate from host CUDA
  driver policy and host graphics wrapper policy.
- `robo-nix` may select a host graphics wrapper, but it must not become one.
  `hostGraphics = "auto"` is the default, uses `/run/opengl-driver` on NixOS
  hosts, and uses the generic robo-provided nixGL wrapper on other Linux hosts.
  Keep `null` as an explicit opt-out.
- `hostGraphics = "nixgl-nvidia"` must require the NVIDIA nixGL wrapper and may
  use `ROBO_NIX_NVIDIA_VERSION` when host driver version detection is
  unavailable. Do not keep legacy aliases for host graphics policies.
- Let nixGL wrappers own graphics wrapper variables. Do not add robo-owned
  PRIME render-offload defaults on top of nixGL output.
- Do not maintain Rust-owned GLX/EGL/GBM host graphics wrapping now that host
  graphics is delegated to nixGL or `/run/opengl-driver`.
- Do not add generated-shell scans over host CUDA, WSL, or distro driver
  directories. Host CUDA driver bridging is Rust-owned and may
  use the reviewed probe path: explicit `ROBO_NIX_LIBCUDA_PATH`,
  inherited `LD_LIBRARY_PATH`, `ldconfig`, and known host driver locations when
  the project appears to need `libcuda.so.1`. Keep `ROBO_NIX_LIBCUDA_PATH` as an
  override and honor `ROBO_NIX_DISABLE_HOST_CUDA_AUTO`. Host NVIDIA graphics
  version probing is allowed only for `hostGraphics = "nixgl-nvidia"` so nixGL
  can be built with the matching driver version.
- Linux input packages such as `evdev` are handled through the `linux-headers`
  component. Keep this as a generic native-header contract, not a
  package-specific workaround.
- `native-build` must expose the C++ runtime library as well as compiler tools.
  Python wheels such as NumPy can import native extensions that need
  `libstdc++.so.6` even when no package is actively compiling.
- `native-build` must expose zlib as a runtime library. Native Python wheels can
  import extensions that need `libz.so.1` even when installation succeeded.
- `native-build` must expose legacy `libcrypt.so.1`. Some proprietary or older
  simulator/runtime extensions still link against the legacy crypt soname.
- `native-build` may expose generic compiler-owned development prefixes such as
  libc for project inspection, but keep those contracts component-level and
  avoid package-specific build handoffs.
- `desktop-gl` must cover GLFW's basic Linux windowing path, including
  `libxkbcommon` for Wayland keyboard support.
- `desktop-gl` must expose generic Linux graphics/display client libraries
  needed by EGL/GLX platform loaders, including `libdrm.so.2`, `libgbm.so.1`,
  `libxcb.so.1`, and `libxshmfence.so.1`.
- `desktop-gl` must expose common legacy X/GL runtime libraries used by large
  simulator stacks, including `libXt.so.6` and `libGLU.so.1`.
- CLI human output should go through the local styled output helpers so labels,
  colors, and non-interactive output stay consistent.
- Keep CLI styling aligned with the original Rust CLI: lowercase section
  headings, cyan phase labels, green success markers, dim field/action labels,
  `indicatif` braille spinners, and a `[robo]` prompt prefix in interactive
  shells.
- Long-running bounded robo commands should use the original-style nested
  progress tree instead of standalone spinners: parent command line, active
  child status, optional dim Nix detail rows, per-step timings, and a completed
  tree in terminals. This applies to `robo shell`, `robo run`, `robo update`,
  and any future command that performs silent setup, Nix work, installation, or
  cache maintenance. Keep non-interactive output as plain direct status lines,
  adding phase labels only when needed to disambiguate otherwise similar work.
- Hide successful Nix CLI output, including dirty Git tree warnings. Capture and
  replay Nix stdout/stderr only when Nix setup fails; after setup succeeds,
  launch the user's shell or command directly with the resolved environment.
- Robo-owned Nix CLI invocations should pass the public robo-nix cache
  substituters and trusted keys directly; do not rely only on host Nix cache
  config or generated flake `nixConfig`.
- Before `nix develop`, runtime setup should best-effort prefetch the dev-shell
  input outputs from configured caches with local builds disabled, then let the
  normal Nix evaluation remain the source of truth.
- The installer-owned flake target is `#robo`; keep that alias available when
  changing package outputs.
- For repeatable local Nix profile installs, prefer `.#robo`. `nix profile add
  .#` installs the default package, but Nix names that profile entry after the
  flake path instead of the package alias, so `nix profile remove robo` may not
  remove it on the next reinstall.
- Generated project `flake.nix` should default `inputs.robo-nix.url` to
  `github:ausbxuse/robo-nix/master` so first bootstrap stays portable and does
  not lock downstream projects to a local checkout or Nix store source. Local
  source testing must opt in with `ROBO_NIX_DEFAULT_SOURCE_URL=path:/...`.
- Package source filtering must explicitly exclude repo-local caches and heavy
  generated directories such as `.robo-nix/`, `target/`, `docs/node_modules/`,
  and VitePress cache/dist outputs; do not rely only on Git ignore behavior for
  store-copy hygiene.

## Verification

Use the narrowest useful checks first:

```bash
cargo test
nix-instantiate --parse flake.nix
```

When generated project files change, also render a temporary project and parse
its generated `flake.nix` and `robo.nix`.

GitHub Actions should stay minimal and mirror real local product checks:
Rust formatting, Rust tests, `nix flake check`, and VitePress docs build/deploy.
Do not wire CI to deleted legacy `tests/` scripts until those scripts are
intentionally restored.
