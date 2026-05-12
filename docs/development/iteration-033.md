# Iteration 033 - Prompt Refresh Final Environment Key

## Goal

Make prompt-time shell refresh store the runtime input fingerprint for the final
environment it exports, including host graphics and CUDA bridge changes.
Keep terminal identity such as `TERM` owned by the user's active terminal rather
than by the captured Nix environment or runtime cache.

## Conflict Check

- Active shell fingerprints should describe the final launched or refreshed
  environment, not the parent process before runtime preparation.
- Host NVIDIA graphics remains explicit policy; do not silently force or remove
  that policy.
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

The first refresh had exported host graphics variables, but the stored runtime
key was computed before those final values were applied.

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

On another user account on the same host, explicit NVIDIA graphics policy
produced a valid NVIDIA GLX context and MuJoCo offscreen readback, but the
interactive MuJoCo window was transparent. The visible GLFW probe used a 32-bit
X visual:

```text
Depth: 32
Visual Class: TrueColor
```

The same application reportedly rendered when launched through `nixGL`, but a
first smoke comparison only tested `nixGL robo run ...`, where `robo` rebuilt
the runtime environment and put its own host graphics variables before the
nixGL library additions.

A focused presentation smoke test then showed that GLFW created an NVIDIA GLX
context and GL readback returned the expected rendered pixel, but the X client
window capture did not contain the rendered stripes. That narrows the failure to
the GLX present path rather than MuJoCo, GLFW initialization, or alpha visuals.
The host exposes `libxshmfence.so.1`, while `desktop-gl` exposed XCB/DRI3
libraries without the shared-memory fence library used by DRI3/Present.

## Scope

- Compute prompt-refresh active shell state from the refreshed environment after
  host graphics and CUDA bridge updates.
- Inherit terminal identity variables from the active process for `robo shell`
  and prompt refresh, and keep them out of the runtime cache.
- Add generic `desktop-gl` runtime libraries required by NVIDIA EGL/GLX platform
  loaders and presentation paths: `libdrm`, `libgbm`, `libxcb`, and
  `libxshmfence`.
- Add a unit test that managed runtime-input environment values are recorded in
  the exported active shell state.

## Non-Goals

- Do not auto-switch explicit NVIDIA graphics policy to Mesa.
- Do not add broad host graphics library directories to `LD_LIBRARY_PATH`.
- Do not mutate an existing project `robo.nix` from `robo shell`.

## Follow-Up

- TODO: make the native Wayland NVIDIA path work without requiring
  `PYGLFW_LIBRARY_VARIANT=x11`. The current failing primitive is below GLFW:
  `wl_display_connect` succeeds, but
  `eglGetPlatformDisplayEXT(EGL_PLATFORM_WAYLAND_EXT, wl_display, ...)` returns
  `EGL_NO_DISPLAY` in the Nix runtime. Keep the current X11/XWayland PyGLFW
  selection as a project workaround until the host NVIDIA Wayland EGL bridge is
  fixed.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
