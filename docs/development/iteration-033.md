# Iteration 033 - Prompt Refresh Final Environment Key

## Goal

Make prompt-time shell refresh store the runtime input fingerprint for the final
environment it exports, including host graphics and CUDA bridge changes.
Keep terminal identity such as `TERM` owned by the user's active terminal rather
than by the captured Nix environment or runtime cache.

## Conflict Check

- Active shell fingerprints should describe the final launched or refreshed
  environment, not the parent process before runtime preparation.
- Host NVIDIA graphics remains explicit policy through `hostGraphics =
  "nvidia"`; do not silently force or remove that policy.
- Keep GLVND/OpenGL dispatch Nix-owned and avoid exposing broad host library
  directories such as `/lib/x86_64-linux-gnu`.

No active review-ledger conflict blocks this pass.

## Failure Observed

After editing `robo.nix` in an active runtime shell, the prompt refresh noticed
the manifest change and exported a refreshed NVIDIA host graphics environment.
The next prompt then immediately refreshed again:

```text
shell: runtime inputs changed
  ! changed ./env:__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS
  ! changed ./env:GBM_BACKENDS_PATH
  ! changed ./env:LD_LIBRARY_PATH
```

The first refresh had exported the host graphics bridge variables, but the
stored runtime key was computed before those final values were applied.

The runtime cache also preserved `TERM=dumb` from environment capture. Launching
an interactive shell from tmux then produced:

```text
$ echo $TERM
dumb
```

even though the parent terminal had `TERM=tmux-256color`.

A separate target-project graphics probe reproduced the MuJoCo/GLFW failure:

```text
GLFWError: (65542) b'EGL: Failed to get EGL display: Success'
window False
```

The NVIDIA host provider was prepared, but the host NVIDIA EGL platform plugins
could not all load inside the runtime:

```text
libnvidia-egl-wayland.so.1: missing libdrm.so.2
libnvidia-egl-gbm.so.1: missing libgbm.so.1
libnvidia-egl-xcb.so.1: missing libxcb.so.1
```

After adding those generic display libraries to `desktop-gl`, the host NVIDIA
plugins loaded. The target GLFW viewer still needed its project-specific
`PYGLFW_LIBRARY_VARIANT=x11` policy because the Wayland PyGLFW path continued to
fail EGL display creation, while the X11 PyGLFW path rendered on the RTX 5090.

## Scope

- Compute prompt-refresh active shell state from the refreshed environment after
  host graphics and CUDA bridge updates.
- Inherit terminal identity variables from the active process for `robo shell`
  and prompt refresh, and keep them out of the runtime cache.
- Add generic `desktop-gl` runtime libraries required by NVIDIA EGL/GLX platform
  loaders: `libdrm`, `libgbm`, and `libxcb`.
- Add a unit test that managed runtime-input environment values are recorded in
  the exported active shell state.

## Non-Goals

- Do not auto-switch `hostGraphics = "nvidia"` to Mesa.
- Do not add broad host graphics library directories to `LD_LIBRARY_PATH`.
- Do not mutate an existing project `robo.nix` from `robo shell`.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
