# Downstream Projects

This document explains the intended consumption model for a project that wants to use `robo-nix`.

## Principle

Do not add every project-specific cross-product environment to `robo-nix`.

Instead:

- `robo-nix` publishes components and generation logic
- your project composes the components it needs
- your project owns its Python dependencies, platform policy, and workspace assumptions

That is what `robo init` and `robo-nix.lib.mkProjectFlake` are for.

The preferred scaling model is now a project-owned manifest, typically named `robo.nix`.

That gives you:

- a small project-local adapter owned by the downstream repo
- generated flake plumbing
- less pressure to add project-specific profiles to the central `robo-nix` repo

## Recommended Entry Point

For most new projects, do not start by copying a central preset name.

Start with the CLI:

```bash
nix run github:ausbxuse/robo-nix#robo -- init .
```

That initializer writes generated flake plumbing plus a project-local manifest. The intended user path is:

- choose a recommended profile
- optionally refine the component list
- generate local runtime files
- use `nix run .#default -- --doctor` and `nix develop`

If inference misses something, pass the missing reusable component directly:

```bash
nix run github:ausbxuse/robo-nix#robo -- init . --with media,qt6
```

## Minimal Project Example

Manifest:

```nix
{
  envName = "project";
  description = "Minimal robot-learning environment";
  components = [
    "base"
    "python-uv"
    "native-build"
  ];
  pythonVersion = "3.11";
  supportedSystems = [
    "x86_64-linux"
    "aarch64-linux"
    "x86_64-darwin"
    "aarch64-darwin"
  ];
  workspaceRoot = ".";
}
```

Thin flake:

```nix
{
  description = "Minimal project using robo-nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    robo-nix.url = "github:ausbxuse/robo-nix";
  };

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}
```

Packaged `robo init` points generated projects at the `robo-nix` source shipped with the CLI package. Use a separate local checkout path only for maintainer testing.

Generated outputs include:

- `apps.<system>.default`
- `apps.<system>.project`
- `devShells.<system>.default`
- `packages.<system>.default`
- `checks.<system>.project`

## Robotics Stack Example

For an Isaac Sim plus ROS 2 project:

```nix
{
  inputs.robo-nix.url = "github:ausbxuse/robo-nix";

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlake {
      envName = "isaac-project";
      description = "Isaac Sim and ROS 2 research environment";
      components = [
        "base"
        "python-uv"
        "native-build"
        "x11-gl"
        "cuda-toolkit"
        "isaac-sim"
        "ros2-jazzy"
        "ros-workspace"
      ];
      pythonVersion = "3.11";
      supportedSystems = [ "x86_64-linux" ];
      workspaceRoot = ".";
    };
}
```

The required workspace shape is then validated by the generated app:

- `ros_ws/src`
- `third_party/isaac-sim`

## Project-Owned Runtime Hooks

Use the central component catalog for reusable runtime needs. Keep project-specific vendor setup in the downstream repo.

`robo.nix` can carry small project-owned extensions:

```nix
{
  envName = "project";
  components = [
    "base"
    "python-uv"
    "native-build"
    "media"
    "x11-gl"
    "qt6"
  ];
  pythonVersion = "3.11";
  supportedSystems = [ "x86_64-linux" ];

  extraPackages = pkgs: [
    pkgs.iproute2
  ];

  requiredDirectories = [
    "third_party/vendor-sdk"
  ];

  shellInit = ''
    export PROJECT_VENDOR_ROOT="$WORKSPACE_ROOT/third_party/vendor-sdk"
  '';

  bootstrap = ''
    . "$WORKSPACE_ROOT/scripts/bootstrap_vendor_sdk.sh"
  '';
}
```

This is the preferred path for repos like Dexmate: `robo-nix` owns the reusable runtime layer, while the downstream repo owns its vendor patches, source tree layout, and bootstrap scripts.

## Dry-Run And Bootstrap

Every generated app supports:

- default mode: bootstrap
- `--dry-run`: validate only
- `--print-config`: print resolved configuration
- `--doctor`: run richer host and workspace diagnostics

Examples:

```bash
nix run .#default -- --dry-run
nix run .#default -- --print-config
nix run .#default -- --doctor
```

## Workspace Selection

`workspaceRoot` declares the default workspace path in the flake.

That can always be overridden at runtime:

```bash
ROBO_NIX_WORKSPACE=/path/to/checkout nix run .#default -- --dry-run
```

This separation is important:

- the environment definition stays declarative
- the actual host checkout can vary safely

## Python Strategy

Use `pythonVersion` to declare the uv-managed Python version for the project.

Example:

```nix
pythonVersion = "3.11";
```

Then keep Python dependencies in standard Python project files:

- `.python-version`
- `pyproject.toml`
- `uv.lock`

Nix provides `uv` and native/runtime dependencies. uv provides the interpreter, virtual environment, and packages.

## Platform Strategy

Declare supported systems explicitly.

Good:

```nix
supportedSystems = [ "x86_64-linux" ];
```

Better than:

- assuming all components work everywhere
- exposing broken outputs on unsupported platforms

## Recommended Project Layout

For a plain Python or research project:

```text
.
├── flake.nix
├── pyproject.toml
└── src/
```

For a ROS 2 workspace:

```text
.
├── flake.nix
└── ros_ws/
    └── src/
```

For Isaac Sim integration:

```text
.
├── flake.nix
├── ros_ws/
│   └── src/
└── third_party/
    └── isaac-sim/
```

## When To Add To `robo-nix`

Add something upstream to `robo-nix` when:

- it is a reusable capability, not just your project shape
- multiple projects could compose it
- it has a clear platform story
- it can carry its own validation behavior

Do not add to `robo-nix` merely because one project combines two existing components.
