# CUDA

CUDA support is split between host reality and reproducible runtime setup.

- `uv.lock` pins Python wheels.
- `robo` can infer the expected CUDA wheel ABI from `uv.lock`.
- Nix provides the selected CUDA toolkit in the runtime shell.
- The host NVIDIA driver remains a host prerequisite.

Run:

```bash
robo check
```

TODO(robo): make CUDA version selection more interactive once the host-driver compatibility policy is fully specified.
