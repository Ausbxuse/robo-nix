# Diagnostics

`robo doctor` is the primary debugging surface.

It explains what `robo` observed, which layer owns the failure, and what command or project file to inspect next.

## Commands

```bash
robo doctor
robo doctor --why
robo doctor --deep
```

Use `--why` to explain selected components. Use `--deep` to run slower runtime and host probes.

## What Doctor Reports

`robo doctor` reports:

- selected runtime components
- expected Python files and uv state
- supported platform
- missing workspace paths
- host prerequisites
- native runtime library issues
- graphics and CUDA facts when deep checks are requested
- project-owned bootstrap scripts and whether they are enabled

It avoids pretending project policy is known. For example, it does not guess uv groups, optional extras, private package indexes, or editable source pins.

## Common Failure Classes

### Old virtualenv

Symptom:

```text
GLIBC_2.38 not found
```

Likely cause: `.venv` was created outside the runtime and is mixing host Python/glibc with Nix native libraries.

Fix:

```bash
robo shell
uv venv --clear
uv sync
```

### Missing editable source

Symptom:

```text
Distribution not found at: file:///.../third_party/...
```

Likely cause: project dependency metadata references a local source checkout that is missing.

Fix the project checkout, submodule, vendored source, or dependency declaration. `robo` does not infer a project-specific install mode.

### Missing native build helper

Symptom:

```text
Could not find pybind11Config.cmake
```

Likely cause: a Python-owned build helper is missing from the active uv environment, or build isolation hides it.

Fix the project's Python build requirements or uv group selection.

### CUDA driver mismatch

Symptom: CUDA wheels load but fail at runtime, or `robo doctor --deep` reports a driver below the required CUDA API version.

Likely cause: host NVIDIA driver does not satisfy the CUDA wheel ABI selected by `uv.lock`.

Fix the host driver or choose project dependencies compatible with the host.

### Graphics/EGL mismatch

Symptom:

```text
EGL: Failed to get EGL display
gladLoadGL error
```

Likely cause: display socket, EGL vendor file, or host/container graphics visibility issue.

Use `robo doctor --deep` to inspect selected `libEGL.so.1`, vendor files, and display variables.

### Bootstrap failure

Project bootstrap scripts are project-owned code. Non-interactive `robo init` records discovered bootstrap scripts as review suggestions instead of enabling them automatically.

A project enables bootstrap only by adding scripts to the `bootstrap` block in `robo.nix` or by passing `--source-script` explicitly.

If bootstrap fails, fix the project script or its required environment variables rather than adding project-specific policy to `robo-nix`.
