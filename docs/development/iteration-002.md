# Iteration 002 - Shell-Centered Bootstrap

## Goal

Replace the iteration-001 `init`/`check` surface with a shell-centered workflow:

```bash
uv python pin <version>
robo shell
uv sync
```

`robo shell` should prepare the minimal Nix runtime files when they are missing,
then delegate to `nix develop`.

## Accepted Scope

- Remove `robo init`.
- Remove `robo check`.
- Keep `robo shell` and `robo run`.
- Require `.python-version`.
- Never create `pyproject.toml`.
- Use checked-in templates embedded with `include_str!`.
- Add minimal first-bootstrap runtime inference from `pyproject.toml`.
- Add a minimal root `flake.nix` for repo toolchains.
- Add minimum user docs, developer docs, and root `AGENTS.md`.

## Bootstrap Contract

`robo shell` owns first bootstrap of:

- `flake.nix`
- `robo.nix`
- `.robo-nix/`

`flake.lock` may be created by normal Nix behavior when `nix develop` needs it.
`robo` should not eagerly refresh it in this iteration.

If `flake.nix` already exists and does not look like a robo project flake,
`robo shell` fails instead of overwriting it.

If `robo.nix` already exists, it is user-managed and canonical. `robo shell`
does not update it or warn about inferred component differences.

## Runtime Inference

Runtime inference is first-bootstrap only. When `robo.nix` is missing and
`pyproject.toml` exists, `robo shell` uses the small data file in
`metadata/runtime-inference.tsv` to choose initial components.

Initial package markers:

- `torch`
- `pytorch`
- `opencv-python`
- `mujoco`

CUDA is reported as a note, not selected automatically.

## Review Notes

Pending concerns from this iteration should be appended here during review and
handled in the next iteration as a coherent change.

## Verification

Run for this iteration:

- `cargo check`
- `cargo test`
- `nix develop --accept-flake-config --command cargo fmt --check`
- `nix-instantiate --parse flake.nix`
- `nix-instantiate --parse templates/project/flake.nix`
- `nix flake check --accept-flake-config`
- Smoke bootstrap in `/tmp/robo-iter2-smoke` using a fake `nix` executable so
  `robo shell` prepared files without entering a real shell.
- `nix-instantiate --parse flake.nix` in the smoke project.
- `nix-instantiate --parse robo.nix` in the smoke project.
- `nix eval --accept-flake-config --no-write-lock-file .#devShells.x86_64-linux.default.name`
  in the smoke project.
- `robo run true` in the smoke project using the same fake `nix` executable.

Pending concerns:

- None yet.
