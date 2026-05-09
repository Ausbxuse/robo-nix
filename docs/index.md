---
layout: home

hero:
  name: robo-nix
  text: Native robot-learning runtime for uv projects.
  tagline: '`robo shell` prepares the Nix layer without taking over Python packaging.'
  actions:
    - theme: brand
      text: Get Started
      link: /users/getting-started
    - theme: alt
      text: Developer
      link: /developers/

features:
  - title: Start from an existing uv project
    details: Pin Python with `uv python pin`, run `robo shell`, then use `uv sync` inside the prepared runtime.
  - title: Get native libraries into Python
    details: Add components such as `native-build`, `linux-headers`, `desktop-gl`, or `cuda-toolkit` in `robo.nix`.
  - title: Fix missing shared libraries
    details: When an import fails on `libassimp.so` or another native library, use `robo search` to find Nix package candidates.
---

<div class="hero-terminal" aria-label="Example terminal session">
  <div class="hero-terminal__bar">
    <span class="hero-terminal__dot"></span>
    <span class="hero-terminal__dot"></span>
    <span class="hero-terminal__dot"></span>
    <span>robot-learning</span>
  </div>
  <pre class="hero-terminal__body"><code><span class="prompt">$</span> uv python pin 3.11
<span class="prompt">$</span> robo shell
<span class="label-note">generated</span>
  <span class="label-ok">✓</span> <span class="dim">wrote</span>    ./flake.nix
  <span class="label-ok">✓</span> <span class="dim">wrote</span>    ./robo.nix
<span class="label-note">inferred</span>
  <span class="label-ok">✓</span> native-build   pyproject.toml dependency `evdev`
  <span class="label-ok">✓</span> linux-headers  pyproject.toml dependency `evdev`
shell: launching zsh
<span class="robo-prompt">[robo]</span> <span class="prompt">$</span> <span class="terminal-cursor"></span></code></pre>
</div>

## Workflow

Install once, then enter the runtime from each project:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ausbxuse/robo-nix/rewrite/scripts/install.sh | sh
uv python pin <version>
robo shell
uv sync
```

`robo shell` prepares the native runtime first, then leaves Python package sync
to the normal uv workflow inside that shell.
