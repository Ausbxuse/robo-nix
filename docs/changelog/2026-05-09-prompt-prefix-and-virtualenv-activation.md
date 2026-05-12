# 2026-05-09 - Prompt Prefix And Virtualenv Activation

## Downstream Failure

Sourcing a Python virtual environment inside an active `robo shell` could show
two robo prompt markers:

```text
[robo](psi) [robo]...
```

The zsh and bash prompt hooks removed an existing `[robo]` marker only when it
was the first token in the prompt. Virtualenv activation can prepend
`(environment) ` ahead of the existing prompt, leaving the old marker behind and
letting the prompt hook add a new one.

## Fix

- Make the bash and zsh prompt prefix helpers remove existing robo markers both
  at the beginning of the prompt and immediately after a leading virtualenv
  prompt segment.
- Allow Python activation scripts to keep showing their virtualenv marker.
- Keep the prompt hook idempotent so repeated prompt refreshes do not accumulate
  markers.

## Verification

- Simulated zsh prompt: `PROMPT="(psi) [robo]%# "; __robo_prompt_prefix`
- Simulated bash prompt: `PS1="(psi) [robo]\\$ "; __robo_prompt_prefix`
- `nix develop -c cargo fmt -- --check`
- `nix develop -c cargo test`
- `nix-instantiate --parse flake.nix`
