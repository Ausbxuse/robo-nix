# robo-nix minimal core

This branch is a greenfield rebuild of `robo-nix` around a shell-centered
workflow for robot-learning projects.

The current product contract is intentionally small:

- `robo shell` prepares missing runtime files and enters `nix develop`.
- `robo run <command> [args...]` prepares the same runtime and runs one command.
- `.python-version` is required.
- `pyproject.toml` is owned by uv/project policy and is never created by `robo`.
- `robo.nix` is created only on first bootstrap; after that it is user-managed.

There is no `robo init`, `robo check`, or `robo diagnose` in this branch.

Install:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/rewrite/scripts/install.sh | sh
```

For a local checkout:

```bash
ROBO_NIX_FLAKE="path:$PWD" ./scripts/install.sh
```

Typical workflow:

```bash
uv python pin <version>
robo shell
uv sync
```

Runtime inference is first-bootstrap only. If `pyproject.toml` exists when
`robo.nix` is missing, `robo shell` uses the small data file in
`metadata/runtime-inference.tsv` to choose initial runtime components.

CUDA and desktop graphics are component-based:

- `desktop-gl` provides Nix-managed desktop graphics libraries.
- `cuda-toolkit` provides the Nix-managed CUDA build toolkit.
- `linux-headers` provides Linux kernel headers for packages such as `evdev`
  that build native input-device extensions.
- Host `libcuda.so.1` still comes from the NVIDIA driver. Set
  `ROBO_NIX_LIBCUDA_PATH` explicitly when a CUDA workload needs that driver
  library inside the runtime.
