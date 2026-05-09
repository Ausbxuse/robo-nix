# Iteration 014 - Minimal Generated Flake

## Goal

Make generated project `flake.nix` match the original product shape: small
plumbing that points at robo-nix and delegates the runtime implementation away
from the user project.

## Scope

- Move project dev-shell construction into `nix/project-flake.nix`.
- Expose `robo-nix.lib.mkProjectFlakeFromManifest` from the repo root flake.
- Change the generated `templates/project/flake.nix` to cache config, one
  `robo-nix` input, and one `outputs` handoff.
- Keep `robo.nix` as the editable project runtime manifest.
- Preserve recognition of older generated flake files so `robo shell` does not
  reject projects created by earlier rewrite iterations.

## Non-Goals

- No migration command for existing large generated flakes.
- No automatic rewrite of existing `flake.nix`.
- No broad component refactor while moving the Nix implementation boundary.

## Review Notes

Pending concerns:

- The default generated `robo-nix` input is `github:ausbxuse/robo-nix/rewrite`.
  Local iteration can override it with `ROBO_NIX_DEFAULT_SOURCE_URL=path:/...`
  before first bootstrap.
- The root flake now carries the `nixpkgs-python` input because downstream
  generated flakes delegate Python interpreter selection to the repo library.

## Verification

Run for this iteration:

- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix-instantiate --parse nix/project-flake.nix`
- smoke bootstrap with `ROBO_NIX_DEFAULT_SOURCE_URL=path:$PWD`
- parse and evaluate the generated smoke flake
