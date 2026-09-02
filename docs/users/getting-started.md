# Getting Started

Start from a Python project directory. uv still owns Python package metadata,
dependency groups, lockfiles, and virtualenv sync.

## What to expect

- `robo shell` is the main command. It prepares the runtime, then opens your
  normal interactive shell with a `[robo]` prompt prefix.
- On first use, `robo shell` may create `flake.nix`, `robo.nix`, and
  `.robo-nix/`. After that, `robo.nix` is yours to edit.
- `robo run [--profile <name>] [--sync] [--] <command> [args...]` uses the same
  runtime preparation path for one command without keeping a shell open.
- `robo search <library>` helps find Nix package candidates for missing shared
  libraries, such as `libassimp.so`. It only prints suggestions.
- `robo refresh` clears robo-owned runtime state under `.robo-nix/`. In an
  active runtime shell, the shell updates at the next prompt.
- In a project, `robo update` updates the `robo-nix` flake input, reinstalls
  the `robo` CLI binary from that input, and clears runtime cache state. In the
  `robo-nix` source checkout, it reinstalls the binary from `.#robo` without
  updating unrelated flake inputs. It does not update Python dependencies or
  `robo.nix`.
- `robo --help`, `robo --version`, and `robo -V` are available as standard CLI
  utilities.
- Active `robo shell` sessions refresh at the next prompt when runtime inputs
  change. This includes the selected runtime profile, `flake.nix`,
  `flake.lock`, `.python-version`, `pyproject.toml`, `uv.lock`, `robo.nix`, and
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

Newly generated project `flake.nix` files point at
`github:ausbxuse/robo-nix/master` by default, even when `robo` was installed
from a local checkout. To test a project against local robo-nix source, set
`ROBO_NIX_DEFAULT_SOURCE_URL=path:/path/to/robo-nix` before first bootstrap.
After changing that override, rebootstrap the test project by removing generated
runtime files such as `flake.nix`, `flake.lock`, `robo.nix`, and `.robo-nix/`,
then running `robo shell` again.

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
robo shell --sync
```

On first bootstrap, `robo shell` may create:

- `flake.nix`: minimal Nix plumbing that delegates to robo-nix.
- `robo.nix`: the project runtime manifest.
- `.gitignore`: in Git worktrees, an entry for `.robo-nix/` when missing.

With `--sync`, robo runs `uv sync --locked` in the prepared runtime before
entering your default interactive shell with a `[robo]` prompt prefix. Set
`ROBO_NIX_SHELL` only when you need to override shell selection.

## 4. Sync Python packages

```bash
uv sync
```

You can also run `uv sync` manually inside the prepared shell. Native Python
extensions can then see the runtime libraries and headers exposed by Nix. In
profile-based projects, robo sets `UV_PROJECT_ENVIRONMENT` to a profile-specific
virtualenv under `.robo-nix/venvs/<profile>/`, so different runtime profiles can
stay installed side by side.

## 5. Adjust runtime components

After first bootstrap, edit `robo.nix` for project runtime choices such as
native build tools, Linux headers, desktop graphics, Qt, or CUDA build tooling.

For example, a project using `evdev` and GLFW-style windows usually needs:

```nix
{
  defaultProfile = "default";

  profiles = {
    default = {
      components = [
        "python-uv"
        "native-build"
        "linux-headers"
        "desktop-gl"
      ];

      pythonExtras = [];
      pythonGroups = [];

      extraPackages = pkgs: [
      ];

      extraRuntimeLibraries = pkgs: [
      ];

      hostGraphics = "auto";
    };
  };
}
```

Projects that contain multiple deployable surfaces can define more profiles in
the same `robo.nix`:

```nix
{
  defaultProfile = "workstation";

  profiles = {
    workstation = {
      components = [ "python-uv" "native-build" "linux-headers" "desktop-gl" ];
      pythonExtras = [ "workstation" ];
      pythonGroups = [ "dev" ];
      hostGraphics = "auto";
    };

    tianji-driver = {
      components = [ "python-uv" "native-build" "linux-headers" ];
      pythonExtras = [ "tianji-driver" ];
      pythonGroups = [];
      hostGraphics = null;
    };
  };
}
```

Use a non-default profile with:

```bash
robo shell --profile tianji-driver --sync
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
robo run [--profile <name>] [--sync] [--] <command> [args...]
```

Use the optional `--` when the command name starts with `-`. Any later `--`
belongs to the command being run. With `--sync`, robo runs `uv sync --locked`
before launching the child command.

Clear local robo runtime state and rebuild the active shell environment at the
next prompt:

```bash
robo refresh
```

Update robo-nix tooling and the installed CLI:

```bash
robo update
```

In a robo project, this updates the `robo-nix` input in `flake.lock`,
reinstalls the `robo` CLI binary from that updated input, and clears runtime
cache state. The next `robo shell` or `robo run` uses the updated lock. In a
local `robo-nix` source checkout, the same command reinstalls `.#robo`; the
checkout has no child `robo-nix` lock input, and robo does not try to update
its other inputs or Git state.

When an official installed CLI is newer than a project's locked official
`robo-nix` revision, the first `robo shell` or `robo run` for that CLI/lock
pair automatically makes one best-effort update of the declared input. A
network failure is reported as a warning and runtime startup continues with
the existing lock. Robo preserves existing runtime caches during this
automatic step, so last-working offline fallback remains available. Run
`robo update` later to retry explicitly.

## What robo does not do

`robo` does not create `pyproject.toml`, run `uv sync` unless `--sync` is
explicitly requested, resolve Python packages, update project dependencies, or
rewrite `robo.nix` after first creation.
