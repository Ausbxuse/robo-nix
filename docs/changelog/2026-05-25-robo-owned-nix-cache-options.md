# Robo-Owned Nix Cache Options

## Context

A generated project flake already advertises the public robo-nix caches through
`nixConfig`, but a downstream `robo shell` still attempted to build CPython when
the host Nix configuration did not list those substituters.

The reproduced dry run showed the important difference:

- without explicit Nix options, the shell derivation planned to build
  `python3-3.11.15.drv`;
- with the robo-nix cache passed directly, the CPython output was fetched from
  the cache and only the local shell derivation remained to build.

Follow-up inspection found a second Nix behavior: `python3-3.11.15` had `out`
and `debug` outputs, but the zhenyu cache only contained `out`. `nix develop`
still started a local CPython build. Explicitly realizing the cached `out` path
before `nix develop` avoided the build.

## Review Ledger

Related prior concern:

- `2026-05-08-minimal-generated-flake.md` put cache hints in generated project
  flakes while keeping the flake small.

No conflict blocks this change: generated flakes can keep their cache hints for
portable Nix behavior, while robo-owned runtime commands pass the same cache
settings directly so setup does not depend on host substituter lists.

## Change

- Add a shared Rust helper for robo-owned Nix CLI commands in the existing
  runtime environment module so local Git-backed flake builds do not depend on
  a newly added source file before it is committed.
- Pass the robo-nix public substituters and trusted keys to Nix commands that
  evaluate or run runtime setup.
- Disable stale negative narinfo caching for robo-owned Nix commands so a path
  pushed to the cache can be discovered immediately.
- Before `nix develop`, parse the shell derivation's requested input outputs and
  best-effort prefetch them with `nix build --no-link --keep-going` and
  `max-jobs = 0`, so the prefetch step can copy cached outputs but cannot
  compile uncached ones.
- Run an internal runtime-prefetch command and `nix develop` as steps inside
  the same progress tree so terminals show spinner feedback immediately instead
  of appearing stuck before the runtime shell animation starts.
- Keep successful Nix output hidden behind the existing progress UI.

## Verification

- [x] `nix develop --command cargo fmt --check`
- [x] `cargo test`
- [x] `nix-instantiate --parse flake.nix`
- [x] `nix build .#robo --no-link`
- [x] downstream probe: explicit `nix-store --realise` fetched
  `/nix/store/mvm554m4bv76rsvw86j5nm95m68aixp6-python3-3.11.15` from
  `https://cache.zhenyuzhao.com/zhenyu-public`, and `robo run true` then
  completed without a CPython build
- [x] after installing the updated binary, `robo refresh && robo run true` in
  the downstream project printed `prefetching runtime paths` and completed
  without a CPython build
- [x] smoke `nix --option extra-substituters ... --option
  extra-trusted-public-keys ... eval --impure --raw --expr
  builtins.currentSystem`
