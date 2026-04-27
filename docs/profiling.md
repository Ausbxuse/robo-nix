# Profiling

This document covers how to profile `robo-nix` itself.

## What To Profile

The most useful profiling targets in this repo are evaluation paths, not build throughput.

In practice, the important maintainer-facing paths are:

- `nix eval` of common outputs
- `nix flake show --all-systems`
- `nix flake check`

Those are the paths that reveal whether the flake structure is staying cheap to inspect and maintain.

## Built-In Profiling Command

The repo exposes:

```bash
nix run .#repo-profile
```

That command times:

- `nix eval --raw path:$PWD#apps.x86_64-linux.default.program`
- `nix eval --raw path:$PWD#packages.x86_64-linux.default.name`
- `nix flake show path:$PWD --all-systems`

It is intentionally simple and repeatable.

## Current Baseline On This Host

During the latest hardening pass, the measured timings on this host were approximately:

- default app eval: `2.41s`
- `flake show --all-systems`: `4.79s`

Treat those as local reference numbers, not universal targets. They will vary with machine, cache state, Nix version, and network/store conditions.

## When To Re-Profile

Re-run profiling when:

- changing the generator in `lib/mk-flake-from-envs.nix`
- adding many new presets
- adding more systems or heavy runtime components
- changing top-level flake wiring
- introducing new repo-level checks or maintainer tools

## Interpreting Regressions

If evaluation gets slower, look first at:

- repeated normalization work inside per-variant generation
- repeated `lib.unique` or list concatenation on the same data
- accidental forcing of large attrsets
- expensive top-level per-system glue in `flake.nix`
- wider preset catalogs that should instead be downstream composition

## Recent Refactor Direction

The current generator was refactored to reduce repeated per-variant work by:

- normalizing `pythonVersion` and `supportedSystems` early
- normalizing merged component data once per variant
- reusing helper renderers for required paths and printed config
- splitting data shaping from shell-script assembly

That makes the code easier to maintain and reduces avoidable repeated list work.

## Profiling In CI

The repo does not currently fail CI on absolute timing thresholds.

That is deliberate:

- CI machines are noisy
- caches vary
- timing regressions should be interpreted by humans before turning into hard gates

For now, profiling is a maintainer tool and validation-covered command, not a benchmark gate.
