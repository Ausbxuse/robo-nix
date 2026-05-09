# Repository Guidance

`robo-nix` is a focused runtime environment tool for robot-learning projects.
It should be robust, easy to use, and narrow. Do not grow it into a general
environment manager.

## Current Product Shape

- `robo shell` is the canonical user command.
- `robo run <command> [args...]` uses the same runtime preparation path as
  `robo shell`.
- There is no `robo init`, `robo check`, or `robo diagnose` in the current
  branch.
- `robo shell` may create missing runtime files during first bootstrap, then
  enters `nix develop`.
- `.python-version` is required. `robo` does not choose a default Python
  version.
- `pyproject.toml` is owned by uv/project policy. `robo` must never create it.
- `robo.nix` is user-editable after first creation and is canonical for the
  shell.

## Ownership Boundaries

- uv owns Python package metadata, dependency groups, extras, virtualenv sync,
  and lockfiles.
- Nix owns the CPython interpreter, native tools, runtime libraries, and shell
  environment.
- Rust owns command UX, diagnostics, project-file preparation, and command
  wrapping.
- Runtime inference rules should live in data files, not hardcoded Rust
  conditionals.
- Docs must describe the current minimal branch. Do not reintroduce `robo init`,
  `robo check`, or `robo diagnose` in user-facing docs until those commands are
  intentionally restored.
- Keep docs Node tooling under `docs/`; do not add root Node package files.
- Installer docs and scripts must end with the current `robo shell` workflow.
  Do not leave stale installer text pointing users at removed commands.

## Iteration Rules

- Keep each iteration small enough to review line by line.
- For concrete downstream failures with logs or a known command, reproduce the
  failure before coding whenever practical. Record the failing command and key
  error in the iteration doc before or alongside the fix.
- Record review concerns in `docs/development/iteration-*.md` before turning
  them into code.
- Promote only durable operating rules into this file.
- Before starting an implementation iteration, check for conflicts in the review
  ledger and call them out.

## Editing Rules

- Keep generated text in checked-in template/resource files. Rust may embed
  those files with `include_str!`.
- Avoid large raw generated strings in Rust.
- Use comments only when they clarify ownership, incomplete behavior, or a
  future hazard. Prefer specific markers such as `NOTE`, `TODO`, `FIXME`,
  `WARN`, `BUG`, and `DEBUG`.
- Do not overwrite a non-robo `flake.nix`.
- Do not rewrite an existing `robo.nix` from `robo shell`.
- Keep Nix-managed desktop graphics separate from host NVIDIA driver policy.
- Do not add generated-shell scans over host CUDA, NVIDIA, EGL, Vulkan, WSL, or
  distro driver directories. If host CUDA is needed in this branch, honor an
  explicit `ROBO_NIX_LIBCUDA_PATH` and leave broader detection to a reviewed
  future iteration.
- Linux input packages such as `evdev` are handled through the `linux-headers`
  component. Keep this as a generic native-header contract, not a downstream
  project workaround.
- `native-build` must expose the C++ runtime library as well as compiler tools.
  Python wheels such as NumPy can import native extensions that need
  `libstdc++.so.6` even when no package is actively compiling.
- `native-build` must expose zlib as a runtime library. Native Python wheels can
  import extensions that need `libz.so.1` even when installation succeeded.
- `desktop-gl` must cover GLFW's basic Linux windowing path, including
  `libxkbcommon` for Wayland keyboard support.
- CLI human output should go through the local styled output helpers so labels,
  colors, and non-interactive output stay consistent.
- The installer-owned flake target is `#robo`; keep that alias available when
  changing package outputs.

## Verification

Use the narrowest useful checks first:

```bash
cargo test
nix-instantiate --parse flake.nix
```

When generated project files change, also render a temporary project and parse
its generated `flake.nix` and `robo.nix`.
