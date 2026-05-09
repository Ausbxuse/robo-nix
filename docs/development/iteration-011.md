# Iteration 011 - Rewrite Branch And Profile Install

## Goal

Rename the working branch to `rewrite` and make local install guidance match
Nix profile behavior.

## Scope

- Rename the current branch from `greenfield/minimal-core` to `rewrite`.
- Update installer and docs URLs to use the `rewrite` branch.
- Ignore repo-local `.robo-nix/` generated cache.
- Verify the exact `nix profile remove robo && nix profile add .#` behavior.

## Review Notes

Nix profile naming is the important caveat:

- `nix profile add .#robo` creates a profile entry named `robo`.
- `nix profile add .#` installs the default package, but Nix names the profile
  entry after the flake path, such as `robo-nix-minimal`.
- Because of that upstream Nix behavior, `nix profile remove robo && nix profile
  add .#` works when the current profile entry is already named `robo`, but it
  is not repeatable after a `.#` install unless the user also removes
  `robo-nix-minimal`.

## Verification

Run for this iteration:

- isolated profile test for `nix profile remove robo && nix profile add .#`
- real profile test for `nix profile remove robo && nix profile add .#`
- `sh -n scripts/install.sh`
- `npm --prefix docs run build`
