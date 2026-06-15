# Update progress tree

`robo update` used standalone spinners for long Nix work:

```text
⠧ installing robo CLI binary
```

That made it less consistent than `robo shell` and `robo run`, which leave a
completed progress tree with step durations and useful Nix detail rows.

## Review concerns

- `2026-06-12-robo-update.md` intentionally keeps `robo update` narrow: update
  the workspace `robo-nix` input, reinstall the CLI from that input, and clear
  robo-owned runtime cache state. This change keeps that scope and only changes
  progress rendering.
- CLI UX requires long silent work to use the nested progress tree in terminals
  while keeping non-interactive output plain.

No conflict blocks this change.

## Change

- Run `robo update` bounded work under one progress tree.
- Show separate timed steps for updating the flake input, installing the CLI
  binary, and clearing runtime cache state.
- Keep replaying captured Nix output only on failure.
