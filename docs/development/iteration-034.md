# Iteration 034 - Explicit nixGL Host Graphics Wrapper

## Goal

Add an explicit `hostGraphics = "nixgl"` policy for non-NixOS desktop graphics
sessions where nixGL already supplies the correct host OpenGL environment.

## Conflict Check

- Host graphics wrapper selection must stay explicit manifest policy.
- Keep Nix-managed desktop graphics separate from host NVIDIA driver policy.
- Do not turn `desktop-gl` into a broad host driver scanner.
- The existing explicit NVIDIA graphics policy remains a curated robo-owned
  provider in this historical iteration.

No active review-ledger conflict blocks adding a separate explicit nixGL policy.

## Failure Observed

On the same Ubuntu host:

- `hostGraphics = null` selected Mesa GLVND defaults and failed GLFW with
  `GLX: No GLXFBConfigs returned`.
- Explicit NVIDIA graphics policy selected robo's NVIDIA bridge and created an
  NVIDIA GLX context, but the interactive window could remain transparent.
- Running through nixGL was reported to make the application render correctly.

The clean boundary is to let robo continue to own Python, native libraries, and
runtime shell setup, while nixGL owns host OpenGL dispatch when explicitly
selected.

## Scope

- Accept `hostGraphics = "nixgl"` in project manifests.
- Accept `hostGraphics = "nixgl-nvidia"` for projects that require the NVIDIA
  nixGL wrapper and should fail instead of falling back to Mesa.
- During shell setup, find `ROBO_NIX_NIXGL` or a `nixGLNvidia`/`nixGL`/
  `nixGLMesa` executable on `PATH`.
- Import only graphics-related variables from `nixGL env -0`.
- Query nixGL with inherited graphics variables scrubbed so active shell refresh
  cannot feed stale Mesa or NVIDIA wrapper state back into the wrapper.
- Track nixGL-controlled graphics variables in active-shell refresh state.
- Update the generated `robo.nix` comments.

## Non-Goals

- Do not auto-select nixGL.
- Do not vendor nixGL into robo-nix.
- Do not remove the curated explicit NVIDIA graphics policy in this iteration.
