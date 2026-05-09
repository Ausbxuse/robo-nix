---
layout: home

hero:
  name: robo-nix
  text: Robot-learning runtimes without learning Nix first.
  tagline: '`robo shell` prepares the native layer that uv-installed robotics packages need.'
  actions:
    - theme: brand
      text: Install
      link: /users/install
    - theme: alt
      text: Get Started
      link: /users/getting-started
    - theme: alt
      text: Runtime Components
      link: /users/runtime

features:
  - title: uv owns Python
    details: Keep dependency groups, extras, indexes, locks, and virtualenv sync in the Python project layer.
  - title: Nix owns native runtime
    details: Provide CPython, compilers, Linux headers, graphics libraries, CUDA tooling, and native shared libraries.
  - title: robo owns workflow
    details: 'Bootstrap missing runtime files, wrap `nix develop`, and emit pasteable debug logs when setup fails.'
---

<div class="hero-terminal">
  <div class="hero-terminal__bar">
    <span class="hero-terminal__dot"></span>
    <span class="hero-terminal__dot"></span>
    <span class="hero-terminal__dot"></span>
    <span>robot-learning</span>
  </div>
  <pre class="hero-terminal__body"><code><span class="prompt">$</span> uv python pin 3.11
<span class="prompt">$</span> robo shell
<span class="label-ok">generated</span> flake.nix
<span class="label-ok">generated</span> robo.nix
<span class="label-note">inferred</span> native-build from evdev, torch
<span class="label-note">inferred</span> linux-headers from evdev
<span class="label-note">inferred</span> desktop-gl from mujoco
<span class="dim">running nix develop --accept-flake-config</span><span class="terminal-cursor"></span></code></pre>
</div>

## The Contract

`robo-nix` focuses on the native runtime layer for robot-learning projects.
It is intentionally not a Python package manager and not a general development
environment framework.

Install once:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/greenfield/minimal-core/scripts/install.sh | sh
```

Then the normal project loop is:

```bash
robo shell
uv sync
```

Use `robo run <command>` when you want one command to run inside the same
runtime without staying in an interactive shell.
