<div align="center">

# robo-nix

Native runtime environment for uv-based robot-learning projects.

<a href="https://ausbxuse.github.io/robo-nix/"><img alt="Docs" src="https://img.shields.io/badge/docs-online-6fb0f4?style=for-the-badge&labelColor=2c3144&color=6fb0f4"></a>
<a href="https://github.com/ausbxuse/robo-nix/actions/workflows/ci.yml?query=branch%3Amaster"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ausbxuse/robo-nix/ci.yml?branch=master&style=for-the-badge&label=ci&labelColor=2c3144&color=62bcc6"></a>
<a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-GPL--3.0-5d6784?style=for-the-badge&labelColor=2c3144&color=5d6784"></a>

</div>

`robo-nix` helps robot-learning repositories provide the native runtime layer
that Python packaging does not own: CPython, CUDA/toolchain pieces, desktop
graphics libraries, Linux headers, compiler/runtime libraries, and other
system dependencies.

Use `uv` for Python packages and virtual environments. Use `robo` for the
Nix-managed interpreter and native runtime needed to make those packages build
and import reliably.

The command surface is intentionally small. The primary workflow is:

```bash
robo shell
uv sync
```

## Quick Start

Install `robo`:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/master/scripts/install.sh | sh
```

Install from a local checkout:

```bash
ROBO_NIX_FLAKE="path:$PWD" ./scripts/install.sh
```

Manual local profile install:

```bash
nix profile remove robo || true
nix profile add .#robo
```

When `robo` is installed from a local checkout through Nix, newly generated
project `flake.nix` files use that installed checkout snapshot as their
`robo-nix` input. To rebootstrap a test project against the latest local
install, remove its generated runtime files and run `robo shell` again.

Enter a robot-learning project:

```bash
cd robot-learning-project

# Required once if the project does not already pin Python.
uv python pin 3.11

# Create missing robo runtime files on first use, then enter the runtime shell.
robo shell

# Python dependency sync remains project-owned.
uv sync

# Run project commands inside the prepared runtime.
python train.py
```

Leave the runtime shell with `exit`.

## Existing Projects

For a repository that already has `.python-version`, `flake.nix`, and
`robo.nix`, enter the runtime directly:

```bash
robo shell
uv sync
```

For a Python repository without robo runtime files, `robo shell` creates the
missing runtime files on first use. If `pyproject.toml` already exists, the
initial `robo.nix` is inferred from `src/metadata/runtime-inference.tsv`.

After first creation, `robo.nix` is user-managed. `robo shell` does not rewrite
it.

## Commands

```bash
robo shell
```

Prepares the runtime and launches your default interactive shell with a
`[robo]` prompt prefix.

```bash
robo run <command> [args...]
```

Runs one command inside the prepared runtime without opening an interactive
shell.

```bash
robo search <library>
```

Searches Nix package metadata for packages that may provide a missing shared
library, such as `libassimp.so` or `libz.so`.

The public command surface is intentionally limited to runtime shell, runtime
command execution, and shared-library lookup. Setup diagnostics are part of the
shell/run workflow.

## Runtime Model

`robo-nix` keeps ownership boundaries explicit:

- `uv` owns Python package metadata, dependency groups, extras, virtualenv sync,
  and lockfiles.
- Nix owns CPython, native tools, runtime libraries, CUDA/graphics/toolchain
  pieces, and the shell environment.
- `robo` owns the user-facing workflow, generated runtime files, command
  wrapping, and diagnostics.

The generated project `flake.nix` stays small and delegates runtime complexity
to `robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix`.

## Runtime Components

Common components include:

| Component       | Purpose                                                          |
| --------------- | ---------------------------------------------------------------- |
| `python-uv`     | CPython and uv integration for uv-managed projects.              |
| `native-build`  | Compiler/build tools plus C++, zlib, and legacy crypt runtime libraries. |
| `desktop-gl`    | OpenGL/EGL/Vulkan/X11/Wayland libraries for MuJoCo and desktop viewers. |
| `linux-headers` | Kernel headers for Linux input packages such as `evdev`.         |
| `cuda-toolkit`  | Nix-managed CUDA toolkit for native builds and CUDA packages.    |

Host NVIDIA driver libraries remain host-owned. For projects that appear to
need host `libcuda.so.1`, `robo shell` and `robo run` probe the host and add a
visible driver library automatically. Set `ROBO_NIX_LIBCUDA_PATH` to override
the detected library, or `ROBO_NIX_DISABLE_HOST_CUDA_AUTO=1` to disable the
automatic bridge.

Host graphics provider selection is explicit project policy. Use
`hostGraphics = "nvidia";` in `robo.nix` when a project such as Isaac Sim needs
the host NVIDIA Vulkan/EGL/GLX provider. Leave `hostGraphics = null;` when the
host session should choose the graphics provider.

## Diagnostics

`robo` hides successful Nix setup output so routine warnings such as dirty Git
tree notices do not distract users. If Nix setup fails, `robo` replays the
captured Nix output and writes `.robo-nix/last-error.log` for issue reports.
Every `robo shell` and `robo run` attempt also writes `.robo-nix/last-run.json`
with redacted facts, decisions, environment variable names, and errors.

Python dependency failures remain project-owned. Run `uv sync` explicitly so
the project controls dependency groups, extras, private indexes, editable
sources, and install policy. During first bootstrap, `robo` statically reads
local path dependency metadata from `pyproject.toml` and package names from an
existing `uv.lock` when it can and reports where inference stops, but it does
not fetch remote package metadata or resolve the Python dependency graph.

## Documentation

Read the documentation site at <https://ausbxuse.github.io/robo-nix/>.
It is built and deployed by the `Docs` GitHub Actions workflow from `master`.

Useful local entry points:

- [Getting started](./docs/users/getting-started.md)
- [Runtime support](./docs/users/runtime.md)
- [Troubleshooting](./docs/users/troubleshooting.md)
- [Developer overview](./docs/developers/overview.md)
- [CLI UX contract](./docs/developers/cli-ux.md)

Run the docs locally:

```bash
cd docs
npm ci
npm run dev
```

## Contributing

Contributions should keep the product narrow, reviewable, and verified. Prefer
small changes that improve the `robo shell` and `robo run` runtime workflow
before adding new command surfaces.

AI-assisted development is allowed, but it must be transparent:

- understand and review the generated code before submitting it
- include the smallest verification that proves the change
- disclose AI assistance in non-trivial pull requests
- do not merge behavior that cannot be explained without trusting the model

Example disclosure:

```text
AI-assisted: yes. Used an agent to draft the patch.
Reviewed manually. Verified with `cargo test` and `nix flake check`.
```

## Related Projects

| Project                                                    | Relationship                                                                    |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [Nix](https://nixos.org/)                                  | Reproducible interpreter, native runtime dependencies, and shell environments.  |
| [uv](https://github.com/astral-sh/uv)                      | Python version requests, packages, virtual environments, and lockfiles.         |
| [nixpkgs-python](https://github.com/cachix/nixpkgs-python) | Cached CPython interpreter coverage for uv-managed projects.                    |
| [nixGL](https://github.com/nix-community/nixGL)            | Reference point for host graphics driver bridging.                              |
| [uv2nix](https://github.com/pyproject-nix/uv2nix)          | Nix-native Python packaging for projects that want Nix to own Python packages.  |

## License

`robo-nix` is licensed under GPL-3.0-only. See [LICENSE](./LICENSE).
