# Runtime Profiles

## Context

Some robot-learning repositories contain multiple deployable surfaces in one
workspace. In `dexmate-teleop`, workstation/operator code and robot driver code
share a repository, but the robot driver should not install operator-only Python
packages or graphics runtime libraries.

The desired workflow is profile-first and single-file:

```nix
{
  defaultProfile = "workstation";

  profiles = {
    workstation = {
      components = [ "python-uv" "native-build" "linux-headers" "desktop-gl" ];
      pythonExtras = [ "workstation" ];
      pythonGroups = [ "dev" ];
      hostGraphics = "auto";
    };

    tianji-driver = {
      components = [ "python-uv" "native-build" "linux-headers" ];
      pythonExtras = [ "tianji-driver" ];
      pythonGroups = [];
      hostGraphics = null;
    };
  };
}
```

Then:

```bash
robo shell
robo shell --profile tianji-driver
robo run --profile tianji-driver -- python -m dexmate.driver
robo refresh --profile tianji-driver
```

## Review Ledger

Related prior concerns:

- `robo.nix` is user-editable and canonical after first creation.
- uv owns Python dependency metadata, groups, extras, virtualenv sync, and
  lockfiles.
- Runtime inference rules should stay narrow and data-driven.

No conflict blocks adding runtime profiles as manifest selection. Robo can
select a uv sync policy and profile-specific virtualenv target without resolving
Python dependencies or mutating `pyproject.toml`/`uv.lock`.

## Change

- Add profile selection for `robo shell`, `robo run`, and `robo refresh`.
- Make generated `robo.nix` profile-first while keeping legacy top-level
  manifests valid.
- Resolve `defaultProfile` from `robo.nix` when no profile is requested.
- Use profile-specific runtime cache and virtualenv state.
- Export uv wrapper defaults from profile `pythonExtras` and `pythonGroups` so
  plain `uv sync` uses the selected profile policy.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated temporary project parses `flake.nix` and `robo.nix`
- [x] `nix build .#checks.x86_64-linux.default --no-link`
