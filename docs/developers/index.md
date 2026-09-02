# Developer

`robo-nix` is focused around one user workflow: prepare a robot-learning
runtime with `robo shell`, then let uv manage Python inside that runtime.

Current command surface:

- `robo shell`: bootstrap missing runtime files, evaluate the Nix dev-shell
  environment, then launch the user's interactive shell with that environment.
- `robo run [--profile <name>] [--] <command> [args...]`: use the same
  bootstrap and environment path, then run one command with the resolved
  runtime environment. A single leading `--` after `run` options is stripped as
  the wrapper separator; later separators remain part of the child argv.
- `robo search <library>`: look up Nix package candidates for missing shared
  libraries. It does not edit project files.
- `robo refresh`: clear robo-owned runtime state under `.robo-nix/` and request
  prompt-time environment refresh inside an active runtime shell.
- `robo update`: in a downstream project, update the workspace `robo-nix`
  flake input, reinstall the CLI from that input, and clear runtime cache
  state; in the `robo-nix` checkout, reinstall the CLI from `.#robo` without
  updating the source flake's other inputs.
- `robo __shell-refresh <shell>`: hidden prompt-hook helper used by active
  `robo shell` sessions.

The public command surface is intentionally limited to the commands above.
`robo update` is a self-update helper for the project runtime tooling and CLI,
not a general Nix or Python dependency updater. Global utility flags such as
`robo --help`, `robo --version`, and `robo -V` do not add runtime workflows.
Official builds include source revision metadata so runtime commands can make
a one-time, best-effort update when the running CLI's source commit timestamp
is later than the project lock's. That automatic step is non-fatal and retains
previous caches for offline fallback.

Read next:

- [Overview](./overview.md): architecture, ownership, generated files, and
  verification.
- [CLI UX](./cli-ux.md): output style, progress tree, shell launch, and prompt
  refresh behavior.
