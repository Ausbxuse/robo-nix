# Venv Bin Path Before Sync

## Context

After the runtime profile rewrite, profile-based projects use a profile-specific
uv environment such as `.robo-nix/venvs/operator`. In `dexmate-teleop`, the
normal workflow entered `robo shell`, ran `uv sync`, then tried:

```bash
we teleop --profile tianji_local_keyboard --input-source pico_controller --driver-address 192.168.1.40
```

The command failed with:

```text
we: command not found
```

`uv run we --help` worked, and the script existed under
`.robo-nix/venvs/operator/bin/we`.

## Change

- Always prepend `$UV_PROJECT_ENVIRONMENT/bin` to `PATH` in the runtime shell,
  even before the virtualenv directory exists.
- Always align `VIRTUAL_ENV` with `UV_PROJECT_ENVIRONMENT` for the runtime
  shell.

This keeps console scripts created by a later `uv sync` visible in the same
interactive shell.

## Verification

- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] `cargo test`
- [x] temp profile project with no venv yet has `$UV_PROJECT_ENVIRONMENT/bin` on `PATH`
