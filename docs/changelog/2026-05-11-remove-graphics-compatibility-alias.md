# 2026-05-11 - Remove Graphics Compatibility Alias

## Goal

Remove the deprecated `hostGraphics = "nvidia"` spelling.

## Conflict Check

- Keep the runtime contract clean and truthful.
- Do not maintain backward compatibility for old graphics bridge vocabulary.
- The durable NVIDIA graphics wrapper spelling is `hostGraphics =
  "nixgl-nvidia"`.
- Do not change default `hostGraphics = "auto"` behavior.

No active review-ledger conflict blocks removing the alias.

## Scope

- Reject `hostGraphics = "nvidia"` in `robo.nix`.
- Remove shell-hook alias rewriting.
- Remove template and docs references to the alias.
- Update tests that treated the alias as selected NVIDIA graphics policy.

## Non-Goals

- Do not remove `hostGraphics = "nixgl-nvidia"`.
- Do not add migration tooling.
- Do not change nixGL wrapper import behavior.

## Verification

- [x] `cargo test`
- [x] `nix develop --impure -c cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] generated project rejects `hostGraphics = "nvidia"`
- [x] `nix flake check --impure`
- [x] docs build
