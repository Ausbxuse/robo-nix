# Remote Generated Flake Default

## Context

Nix-built `robo` packages embedded a `path:` URL to the packaged robo-nix source
as the default generated `robo-nix` input. Local profile installs could therefore
bootstrap downstream projects whose `flake.nix` and `flake.lock` pointed at a
local checkout snapshot or Nix store source instead of the public project source.

## Review Ledger

Conflict found before implementation:

- `AGENTS.md`, `2026-05-08-minimal-generated-flake.md`, and
  `2026-05-09-native-build-shell-ergonomics.md` favored embedding the installed
  source snapshot so local profile reinstall plus rebootstrap tests used the
  same source.

Accepted direction for this change:

- Default generated project flakes to `github:ausbxuse/robo-nix/master` so new
  projects stay portable and standalone by default.
- Keep `ROBO_NIX_DEFAULT_SOURCE_URL=path:/...` as the explicit opt-in path for
  local source testing.

## Change

- Remove the Nix package build-time source URL from the `robo` binary.
- Make bootstrap URL selection prefer only `ROBO_NIX_DEFAULT_SOURCE_URL`, then
  the remote Git URL.
- Update user and developer docs to describe the remote default and local
  override.

## Verification

- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] render a temporary project and parse its generated `flake.nix` and
  `robo.nix`
