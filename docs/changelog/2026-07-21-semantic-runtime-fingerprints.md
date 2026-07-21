# Semantic Runtime Fingerprints

## Concern

The runtime cache hashes complete input-file bytes. Formatting-only TOML/JSON
edits and comment-only Nix edits therefore make `robo shell`, `robo run`, and
active prompt refresh invoke Nix even when the runtime meaning is unchanged.
That unnecessary evaluation also reaches the configured substituters during
runtime prefetch, which is undesirable when the existing cached environment is
still valid or network access is limited.

## Conflict Check

- Runtime cache keys must continue to cover every declared and observed runtime
  input, runtime profile selection, and runtime-affecting environment variable.
- Cache reuse must continue to validate referenced Nix store paths.
- `robo.nix` remains canonical and user-owned; fingerprinting must not rewrite
  project files or infer new runtime policy.
- Invalid TOML/JSON and Nix source the conservative comment scanner cannot
  classify must remain byte-hashed so normalization cannot hide a meaningful
  edit or a syntax error.

No review-ledger conflict blocks semantic normalization before hashing. The
2026-05-08 active-shell review explicitly records comment-normalized
`robo.nix` fingerprinting as unfinished follow-up work.

## Scope

- Canonicalize parseable `pyproject.toml`, `uv.lock`, and `flake.lock` content
  before hashing so comments, formatting, and mapping order do not invalidate
  the runtime cache.
- Ignore line and block comments in Nix runtime inputs while preserving quoted
  strings and all executable source text.
- Fall back to exact byte hashing for invalid TOML/JSON and Nix source that the
  conservative comment scanner cannot safely classify, including files with
  Nix indented strings.
- Keep semantic changes, missing files, profiles, runtime environment inputs,
  manual refresh requests, and missing store paths as cache invalidators.

## Verification

- [x] focused semantic-fingerprint tests
- [x] `cargo test`
- [x] `nix develop --command cargo fmt -- --check`
- [x] `nix-instantiate --parse flake.nix`
- [x] `git diff --check`
