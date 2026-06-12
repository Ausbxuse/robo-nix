# Runtime Git

## Context

On `nvidia@192.168.1.40:~/dexmate-teleop`, `robo.nix` carried a local Git
wrapper:

```nix
(pkgs.writeShellScriptBin "git" ''
  exec /usr/bin/env -u LD_LIBRARY_PATH "$git_bin" "$@"
'')
```

The wrapper worked around host `/usr/bin/git` being launched from a runtime
shell with robo-owned `LD_LIBRARY_PATH` entries. That is a generic host-tool
boundary problem, not a project-specific runtime dependency.

## Review Ledger

Related prior concerns:

- `2026-05-11-prompt-refresh-final-environment-key.md` keeps
  `LD_LIBRARY_PATH` as part of the active runtime input fingerprint because host
  CUDA probing can depend on it.
- `2026-05-09-native-build-shell-ergonomics.md` established that common native
  runtime tools can come from Nix when they are part of the runtime shell
  contract.

No conflict blocks this change. Robo can continue exporting the runtime library
path for Python/native extensions while using a Nix-built Git for shell users
and scrubbing that library path from robo-owned Git probes.

## Change

- Include `pkgs.git` in every generated runtime shell so project Git commands do
  not fall through to host `/usr/bin/git` under robo's runtime library path.
- Remove `LD_LIBRARY_PATH` from robo-owned Git probes used to decide whether the
  workspace flake should be evaluated as `.` or `path:<workspace>`.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix-instantiate --parse src/nix/project-flake.nix`
- [x] temporary project `nix develop --command sh -c 'command -v git'`
      resolved Git under `/nix/store/`
