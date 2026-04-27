# Components

Components are the core extension mechanism in `robo-nix`.

Each component is a Nix function that receives a generation context and returns a fragment describing what it contributes to an environment.

## What A Component Can Contribute

A component may provide:

- `packages`: packages added to the shell and bootstrap app runtime
- `shellInit`: shell environment setup
- `bootstrap`: imperative bootstrap logic used by `nix run`
- `requiredDirectories`: workspace directories that must exist
- `requiredFiles`: workspace files that must exist
- `supportedSystems`: explicit system allowlist
- `check`: extra assertions wired into generated `checks`

## Current Catalog

Core components:

- `base`
- `python-uv`
- `native-build`
- `media`
- `linux-headers`
- `x11-gl`
- `qt6`

ROS components:

- `ros2-jazzy`
- `ros-workspace`

Simulation and GPU components:

- `mujoco`
- `cuda-toolkit`
- `isaac-sim`

## Component Domains

The catalog is split across:

- [components/core.nix](../components/core.nix:1)
- [components/ros.nix](../components/ros.nix:1)
- [components/sim.nix](../components/sim.nix:1)
- [components/common.nix](../components/common.nix:1)

That split is intentional. New components should go into the smallest sensible domain file, not back into one giant registry.

## Context Available To Components

Each component function can access values like:

- `envName`
- `envSpec`
- `system`
- `pkgs`
- `pkgsRos`
- `runtimeLibPath`
- `componentCatalog`
- `lib`

This lets a component declare both pure tooling and context-sensitive checks.

## Example Component

The rough shape is:

```nix
{
  my-component = {pkgs, ...}: {
    packages = [ pkgs.git ];
    shellInit = ''
      export MY_COMPONENT_ENABLED=1
    '';
    supportedSystems = [ "x86_64-linux" ];
    check = ''
      grep -F "component=my-component" "$report"
    '';
  };
}
```

## Writing Good Components

Good components:

- express one capability or one tightly coupled concern
- declare platform limits explicitly
- validate component-specific runtime assumptions
- keep shell behavior deterministic
- avoid embedding unrelated project-specific policy

Weak components:

- mix multiple unrelated subsystems
- assume Linux support without declaring it
- rely on silent mutation with no dry-run validation
- hardcode a project name when the capability is reusable

## Pure Vs Impure Layers

In robotics, some integration remains host-local or manually installed.

`robo-nix` treats these differently:

- pure-ish components: uv, build tools, MuJoCo, ROS underlays
- host integration components: CUDA root discovery, Isaac Sim workspace path
- workspace layout components: expected local directories such as `ros_ws/src` or `third_party/isaac-sim`

That distinction should stay visible in the component design.

Host diagnostics that are part of the product workflow, such as CUDA driver checks, belong in the Rust `robo doctor`. Component Nix should declare the runtime environment; it should not grow product-facing shell diagnostics.

## Adding A New Component

Use this checklist:

1. Pick the correct domain file.
2. Declare `supportedSystems`.
3. Add required files or directories if the component expects them.
4. Keep product-facing diagnostics in the Rust `robo doctor`; avoid adding component `doctor` shell unless there is no practical Rust-side probe.
5. Add a `check` fragment so generated `nix flake check` catches regressions.
6. Add or update a downstream fixture if the behavior affects project composition.
7. Update [docs/components.md](./components.md) and [README.md](../README.md) if the public catalog changed.

## Component Composition Guidance

Treat the component layer as LEGO pieces, but not random ones.

Examples of good composition:

- `base + python-uv + native-build`
- `base + python-uv + native-build + ros2-jazzy + ros-workspace`
- `base + python-uv + native-build + x11-gl + cuda-toolkit + isaac-sim`

If two pieces are always required together for correctness, consider whether there is a missing intermediate component.
