# Iteration 022 - CPython Shared Library Runtime

## Goal

Expose the Nix-managed CPython shared library through the generic `python-uv`
component so native Python packages can embed or bootstrap CPython without
downstream-specific library workarounds.

## Conflict Check

- Nix owns the CPython interpreter and runtime libraries.
- uv owns Python package installation, but does not own the system dynamic
  loader path for Nix-provided CPython.
- `python-uv` is always present in generated projects, so the fix should live
  there rather than in an Isaac Sim-specific rule.

## Failure Observed

An Isaac Sim import failed while Omniverse Kit tried to load CPython by soname:

```text
OSError: libpython3.10.so: cannot open shared object file: No such file or directory
```

The Nix CPython package contained `lib/libpython3.10.so`, but the shell runtime
library path did not include the CPython `lib/` directory. Omniverse then fell
back to a bundled plugin path and hit an unrelated legacy dependency:

```text
OSError: libcrypt.so.1: cannot open shared object file: No such file or directory
```

Manually prepending the Nix CPython `lib/` directory made
`ctypes.CDLL("libpython3.10.so")` succeed and allowed `import isaacsim`.

## Scope

- Add the selected CPython package to the `python-uv` runtime library set.
- Keep the behavior generic to CPython embedding and `ctypes` loading.
- Document that `python-uv` contributes the CPython shared library path.

## Non-Goals

- No Isaac Sim-specific package rule.
- No bundled Omniverse library patching.
- No automatic legacy `libcrypt.so.1` workaround when the Nix CPython path is
  sufficient.

## Verification

- [x] local downstream smoke confirms `ctypes.CDLL("libpython3.10.so")` fails
  before adding CPython `lib/`
- [x] local downstream smoke confirms prepending CPython `lib/` allows
  `import isaacsim`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] local override smoke confirms `python-uv` exposes `libpython3.10.so`
