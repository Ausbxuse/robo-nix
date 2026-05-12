# Iteration 029 - NVIDIA Manifest Paths Across Distros

## Goal

Make explicit NVIDIA host graphics policy work on common NixOS and FHS Linux
hosts without asking users to maintain shell hooks.

## Conflict Check

- `desktop-gl` still must not silently force NVIDIA wrapper selection.
- The previous "no automatic host GPU scan" rule remains valid for implicit
  behavior.
- The explicit NVIDIA host-driver policy may
  select from a short reviewed list of known NVIDIA manifest locations.

## Failure Observed

On an Ubuntu host, the explicit NVIDIA graphics policy exported NixOS-only
manifest paths:

```text
VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
__EGL_VENDOR_LIBRARY_FILENAMES=/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json
```

Those files did not exist. The actual host manifests were:

```text
/usr/share/vulkan/icd.d/nvidia_icd.json
/usr/share/glvnd/egl_vendor.d/10_nvidia.json
```

## Scope

- Keep NVIDIA graphics policy explicit.
- Prefer explicit host graphics manifest overrides when set.
- Otherwise select the first existing NVIDIA manifest from the reviewed NixOS
  and FHS distro candidate paths.
- Keep a deterministic fallback path when no candidate exists so diagnostics
  remain inspectable.
- Update user docs and generated comments.

## Non-Goals

- No automatic NVIDIA policy when `hostGraphics = null`.
- No broad filesystem search over host driver directories.
- No mutation of existing `robo.nix`.

## Verification

- [x] `nix develop -c cargo fmt -- --check`
- [x] `nix develop -c cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] render a temporary project with explicit NVIDIA graphics policy and
  inspect the selected manifest variables
- [x] render a temporary project with explicit Ubuntu-style `/usr/share`
  manifest paths
- [x] `npm --prefix docs run build`
- [x] `nix flake check`
