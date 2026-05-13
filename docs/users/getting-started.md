# Getting Started

Start from a Python project directory. uv still owns Python package metadata,
dependency groups, lockfiles, and virtualenv sync.

## What to expect

- `robo shell` is the main command. It prepares the runtime, then opens your
  normal interactive shell with a `[robo]` prompt prefix.
- On first use, `robo shell` may create `flake.nix`, `robo.nix`, and
  `.robo-nix/`. After that, `robo.nix` is yours to edit.
- `robo run <command> [args...]` uses the same runtime preparation path for one
  command without keeping a shell open.
- `robo search <library>` helps find Nix package candidates for missing shared
  libraries, such as `libassimp.so`. It only prints suggestions.
- Active `robo shell` sessions refresh at the next prompt when runtime inputs
  change. This includes `flake.nix`, `flake.lock`, `.python-version`,
  `pyproject.toml`, `uv.lock`, `robo.nix`, the default `.venv/bin/python`, and
  local `.nix` files imported by `robo.nix` or the project flake.
- Refreshing exports a re-evaluated shell environment. It does not run
  `uv sync` and does not rewrite `robo.nix`.
- First-bootstrap inference reads direct dependencies, optional dependencies,
  dependency groups, local `[tool.uv.sources]` path dependencies, and package
  names from an existing `uv.lock` when available.
- Every runtime attempt writes `.robo-nix/last-run.json` with redacted runtime
  facts, including typed host CUDA and graphics probe summaries. When setup
  fails, `robo` also writes `.robo-nix/last-error.log` with context you can
  paste into an issue.

## 1. Install robo

The installer installs the `robo` CLI through Nix profiles. If Nix is missing,
it uses the Determinate Nix installer first.

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/master/scripts/install.sh | sh
```

When testing from a local checkout:

```bash
ROBO_NIX_FLAKE="path:$PWD" ./scripts/install.sh
```

Manual profile install from a local checkout:

```bash
nix profile remove robo || true
nix profile add .#robo
```

Use `.#robo` instead of `.#` so the profile entry is named `robo` and future
`nix profile remove robo` commands keep working.

Local profile installs embed the installed checkout snapshot as the default
`robo-nix` source for newly generated project `flake.nix` files. After
reinstalling a local checkout, a test project can be rebootstrapped against
that version by removing generated runtime files such as `flake.nix`,
`flake.lock`, `robo.nix`, and `.robo-nix/`, then running `robo shell` again.

Installer overrides:

- `ROBO_NIX_FLAKE` changes the flake installed by the script.
- `ROBO_NIX_NIX_INSTALLER_URL` changes the Nix installer URL used when Nix is
  not already installed.

## 2. Pin Python

```bash
uv python pin <version>
```

`robo` reads `.python-version` and does not choose a default Python version.
For example:

```bash
uv python pin 3.11
```

## 3. Enter the runtime

```bash
robo shell
```

On first bootstrap, `robo shell` may create:

- `flake.nix`: minimal Nix plumbing that delegates to robo-nix.
- `robo.nix`: the project runtime manifest.

It then enters your default interactive shell with a `[robo]` prompt prefix.
Set `ROBO_NIX_SHELL` only when you need to override shell selection.

## 4. Sync Python packages

```bash
uv sync
```

Run `uv sync` inside the prepared shell so native Python extensions can see the
runtime libraries and headers exposed by Nix.

## 5. Adjust runtime components

After first bootstrap, edit `robo.nix` for project runtime choices such as
native build tools, Linux headers, desktop graphics, Qt, or CUDA build tooling.

For example, a project using `evdev` and GLFW-style windows usually needs:

```nix
{
  components = [
    "python-uv"
    "native-build"
    "linux-headers"
    "desktop-gl"
  ];

  extraPackages = pkgs: [
  ];

  extraRuntimeLibraries = pkgs: [
  ];
}
```

If runtime inputs change while `robo shell` is open, the prompt hook refreshes
the shell environment at the next prompt. Refreshing does not rewrite
user-managed `robo.nix`.

If an error names a missing shared library, search for Nix package candidates:

```bash
robo search libassimp.so
```

Use one command inside the runtime without staying in an interactive shell:

```bash
robo run <command> [args...]
```

## What robo does not do

`robo` does not create `pyproject.toml`, run `uv sync` automatically, resolve
Python packages, or rewrite `robo.nix` after first creation.
