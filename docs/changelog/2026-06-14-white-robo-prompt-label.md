# White robo prompt label

The profile-aware prompt marker currently renders `[robo:profile]` with `ro`
white and `bo` cyan. That split-color label is noisier than intended now that
the selected profile is also cyan.

## Review concerns

- `2026-06-14-profile-prompt-prefix.md` added the current profile-aware marker
  and duplicate-prefix stripping. This change keeps that behavior and only
  changes the color of the literal `robo` label.

No conflict blocks this change.

## Change

- Render the literal `robo` prompt label entirely white in zsh, bash, and fish.
- Keep the selected profile cyan and the brackets/separator dim.
