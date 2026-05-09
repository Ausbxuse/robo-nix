# Troubleshooting

The current minimal branch has one primary debugging surface: `robo shell` or
`robo run <command>`. When project setup fails, `robo` writes
`.robo-nix/last-error.log` with pasteable context for an issue.

## Missing .python-version

`robo` does not choose a default Python version.

```bash
uv python pin 3.11
robo shell
```

## Existing non-robo flake

If a repository already has a non-robo `flake.nix`, `robo shell` refuses to
overwrite it. Decide whether this project should use that flake or a robo-nix
generated runtime before continuing.

## Missing Native Library

Errors such as these mean a runtime component is incomplete or missing:

```text
libstdc++.so.6: cannot open shared object file
libz.so.1: cannot open shared object file
Wayland: Failed to load libxkbcommon
```

For broad robotics packages, prefer fixing the component contract instead of
adding package-specific shell hacks. For example:

- `native-build` owns compiler runtime libraries.
- `linux-headers` owns Linux input headers.
- `desktop-gl` owns desktop graphics and GLFW windowing libraries.

## Existing robo.nix

After `robo.nix` exists, `robo shell` uses it as the canonical runtime manifest.
It will not re-infer dependencies or rewrite that file. Edit `robo.nix`
directly when a project needs another component.
