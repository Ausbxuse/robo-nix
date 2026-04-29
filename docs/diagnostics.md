# Diagnostics

`robo check` is the primary debugging surface.

It should explain:

- what runtime components were selected
- what is expected from `uv.lock` and project files
- what host prerequisites are missing
- which failure belongs to Python resolution, Nix runtime setup, or host configuration

TODO(robo): move stable probe logic from `robo-cli` into `robo-checks`.
