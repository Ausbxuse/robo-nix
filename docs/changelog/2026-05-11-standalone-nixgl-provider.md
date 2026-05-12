# 2026-05-11 - Standalone nixGL Provider

## Concern

`hostGraphics = "nixgl-nvidia"` failed in an active runtime shell when the user
did not have `nixGLNvidia` installed in their profile:

```text
robo-nix: hostGraphics = "nixgl-nvidia" requires nixGLNvidia on PATH.
```

That made the graphics provider depend on per-user profile state even though the
manifest selected a robo-owned runtime policy.

## Decision

- Add `nix-community/nixGL` as a `robo-nix` flake input.
- Prefer the bundled nixGL wrapper path for `hostGraphics = "nixgl"` and
  `hostGraphics = "nixgl-nvidia"`.
- Keep `ROBO_NIX_NIXGL` as an explicit override for unusual layouts.
- Run `nix develop --impure` because nixGL's NVIDIA wrapper detects the host
  driver version from the machine.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project smoke test
