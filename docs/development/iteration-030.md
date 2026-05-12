# Iteration 030 - Historical NVIDIA Host Graphics Wrapping

## Goal

Make explicit NVIDIA host graphics policy prepare a usable NVIDIA Vulkan/EGL/GLX
view on common non-NixOS hosts without asking users to maintain shell hooks,
symlink directories, or `LD_LIBRARY_PATH` snippets.

## Conflict Check

- `desktop-gl` still owns Nix-managed generic graphics and windowing libraries.
- Host NVIDIA graphics remains explicit policy;
  this iteration does not infer or silently force NVIDIA wrapper selection.
- Existing NVIDIA manifest selection is not enough on Ubuntu
  when the host ICD references `libGLX_nvidia.so.0` by soname.
- GLVND should stay Nix-owned for generic dispatch libraries. The host-owned
  side is the NVIDIA vendor provider: NVIDIA GLX/EGL libraries, Vulkan/EGL
  manifests, and NVIDIA EGL external platform configs when present.
- Avoid exposing all of `/lib/x86_64-linux-gnu` or another host system library
  directory into the runtime; that can leak host glibc into the Nix runtime.
- Keep uncommon host layouts overrideable, but do not make users maintain
  project shell snippets for reviewed Ubuntu/NixOS layouts.
- Compare against nixGL behavior without making it a runtime dependency in this
  iteration.

## Failure Observed

On a remote Ubuntu host, the explicit NVIDIA graphics policy selected the correct host
manifests:

```text
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/10_nvidia.json
```

The Vulkan ICD referenced `libGLX_nvidia.so.0` by soname. Inside the Nix runtime
that soname was not discoverable, so Isaac logged:

```text
VkResult: ERROR_INCOMPATIBLE_DRIVER
vkCreateInstance failed
Failed to create any GPU devices
Segmentation fault
```

Related GLVND/GLX failures showed up as missing or empty GLX config discovery,
and OpenCV EGL initialization was sensitive to whether NVIDIA EGL vendor
libraries and external platform configs were visible together. A manual
workaround exposed only NVIDIA host libraries through
`.robo-nix/host-graphics/lib` and prepended that directory to `LD_LIBRARY_PATH`.
That passed the downstream datagen check, but it is not an acceptable user
workflow.

## Scope

- Keep explicit NVIDIA graphics as the single user-facing policy for this
  historical implementation.
- After Nix environment capture, resolve reviewed host NVIDIA graphics inputs
  from an explicit override, `ldconfig`, and known NixOS/FHS distro paths.
- Populate `.robo-nix/host-graphics/lib` only with reviewed NVIDIA driver
  library filename families, and require `libEGL_nvidia.so.0` plus
  `libGLX_nvidia.so.0`.
- Populate `.robo-nix/host-graphics/egl_external_platform.d` with NVIDIA EGL
  external platform JSON files when present, and prepend it to
  `__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS`.
- Populate `.robo-nix/host-graphics/gbm` with NVIDIA GBM backends such as
  `nvidia-drm_gbm.so` when present, and prepend it to `GBM_BACKENDS_PATH`.
- Prepend the generated bridge directory to `LD_LIBRARY_PATH` from Rust so
  prompt refresh and `robo run` share the same behavior.
- Record debug/last-run decision lines and warnings when the provider cannot be
  completed.
- Document the intended workflow and override for uncommon host library
  layouts.

## Maintenance Note

Do not rewrite host JSON manifests here. The selected host manifests already
work; the observed gap is GLVND and dynamic-loader visibility for the NVIDIA
vendor provider they reference. Keep the fix to a narrow robo-owned provider
view, not a broad host graphics scan.

Comparing against `nixGL.nix` clarified the runtime contract: the NVIDIA wrapper
puts GLVND plus NVIDIA vendor libraries on `LD_LIBRARY_PATH`, selects NVIDIA EGL
vendor JSON, and selects NVIDIA Vulkan ICD JSON for Vulkan. This bridge follows
that contract with the reviewed host driver layout instead of downloading a
matching Nix `nvidia_x11` userspace package, so diagnostics must stay explicit
and docs must not claim full nixGL equivalence.

## Non-Goals

- No automatic NVIDIA policy when `hostGraphics = null`.
- No broad host driver scan.
- No mutation of existing `robo.nix`.
- No Isaac-specific shell hook or component.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] render a temporary project and parse generated `flake.nix` and `robo.nix`
- [x] unit-test the NVIDIA host graphics wrapping without exposing host glibc
