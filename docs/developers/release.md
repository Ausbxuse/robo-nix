# Release Process

Use SemVer tags with a `v` prefix.

For the first beta series:

```text
v0.1.0-beta.1
v0.1.0-beta.2
v0.1.0-rc.1
v0.1.0
```

## Tag

Create an annotated tag from the commit being released:

```bash
git tag -a v0.1.0-beta.1 -m "robo-nix v0.1.0-beta.1"
```

Push the branch and tag separately:

```bash
git push origin develop
git push origin v0.1.0-beta.1
```

The tag is the version source of truth. A GitHub release is the public release page attached to that tag. Packaged binaries are release assets and can be added later.

## Before Publishing

Run the narrowest checks that prove the release is ready. For a broad release candidate, use:

```bash
bash tests/dev-check.sh
bash tests/full-check.sh
```

If a check requires a specific host, such as GPU validation, say that clearly in the release notes when it was not run.

## Release Notes

Keep release notes short:

- what changed
- what is known to work
- known limitations
- verification run for the release

Do not claim support that was not validated.
