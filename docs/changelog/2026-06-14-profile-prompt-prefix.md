# Profile prompt prefix

## Context

Runtime profiles can now change the selected virtualenv and runtime inputs, but
the interactive shell prompt still showed only `[robo]`. After switching an
active shell to another profile, the visible prompt did not identify which
runtime profile future commands would use.

## Change

- Show the active runtime profile selector in Bash, Zsh, and Fish prompt
  prefixes as `[robo:<profile>]`.
- Keep legacy `[robo]` markers removable so shells created before this change
  do not accumulate duplicate prompt prefixes after refresh.
- Track the last generated Bash and Zsh prompt markers so active profile
  switches replace the previous profile label instead of stacking markers.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
