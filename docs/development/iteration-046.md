# Iteration 046 - Auto nixGL Compatibility

## Goal

Keep generated `hostGraphics = "auto"` projects from evaluating the unpatched
nixGL NVIDIA path on non-NixOS NVIDIA hosts.

## Conflict Check

- Keep `hostGraphics = "auto"` as the generated default.
- Keep delegating non-NixOS graphics wrapping to nixGL.
- Do not restore robo-owned GLX/EGL/GBM wrapping.
- Keep `hostGraphics = "nixgl-nvidia"` as the explicit NVIDIA wrapper policy.

No review-ledger conflict blocks sharing the existing nixGL compatibility patch
with the generic `auto` nixGL wrapper path.

## Reproduction

In `/hfm/zhenyu/SIMPLE`, first `robo shell` generated `flake.nix` and
`robo.nix`, then failed while evaluating the default `hostGraphics = "auto"`
runtime shell:

```bash
robo shell
```

Key error:

```text
building '/nix/store/5apskkzxpmjl1vil193p4gb9fpmzak19-impure-nvidia-version-file.drv'...
error: function 'anonymous lambda' called with unexpected argument 'kernel'
at .../pkgs/os-specific/linux/nvidia-x11/generic.nix:34:1
```

The prior compatibility patch only covered explicit
`hostGraphics = "nixgl-nvidia"`. Generated projects use
`hostGraphics = "auto"`, which still interpolated `nixGLDefault` from the
unpatched nixGL flake package.

## Scope

- Build robo's bundled nixGL wrapper from the patched nixGL source.
- Reuse the same patched source for explicit `nixgl-nvidia`.
- Keep the host graphics policy surface unchanged.

## Non-Goals

- Do not infer `nixgl-nvidia` from NVIDIA hardware.
- Do not change NixOS `/run/opengl-driver` behavior.
- Do not add package-specific graphics rules.

## Verification

- [x] `nix develop --impure -c cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] temporary project with `hostGraphics = "auto"` evaluates the runtime
      shell far enough to select a nixGL wrapper
- [x] `nix fmt -- --check src/nix/project-flake.nix`
