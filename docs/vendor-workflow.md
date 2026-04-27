# Vendor Workflow

`robo vendor` is local-source-first and repo-agnostic.

Curated modules describe how to recognize, install, check, bootstrap, and export vendor source trees. A module may include a public `sourceUrl`; when it does not, `robo vendor` will not fetch anything and will instead tell the user where to place the local checkout.

The default command is:

```bash
robo vendor
```

It detects known vendor trees, clones only modules with an explicit `sourceUrl`, and runs configured bootstrap scripts for detected modules.

`installPath` is the default project-relative location for a source tree. It is not automatically fetched unless the module also has a non-null `sourceUrl`.

Use focused subcommands while debugging:

```bash
robo vendor list
robo vendor add PATH
robo vendor doctor
robo vendor bootstrap
robo vendor export NAME
```

## Adding Modules

Add new curated modules under [`lib/vendor-modules`](../lib/vendor-modules:1), not directly in `lib/vendor-metadata.nix`. Each file should return an attribute set keyed by module name.

Required fields:

- `description`
- `installPath`
- `detectPaths`
- `sourceUrl`, or `null` for proprietary/project-owned sources
- `components`
- `requiredPaths`
- `bootstrapScripts`
- `patches`

Example:

```nix
{
  example-sdk = {
    description = "Example SDK used by a robotics project.";
    installPath = "third_party/example-sdk";
    detectPaths = ["third_party/example-sdk"];
    sourceUrl = "https://example.com/example-sdk.git";
    components = ["native-build"];
    requiredPaths = ["CMakeLists.txt"];
    bootstrapScripts = ["scripts/bootstrap_example_sdk.sh"];
    patches = [];
  };
}
```

`sourceUrl = null` means robo must not fetch it. Use this for proprietary, private, or project-owned source trees.

Only set `sourceUrl` when cloning that source by default is legal, stable, and expected.

## Metadata Fields

| Field | Meaning |
| --- | --- |
| `description` | Human summary shown by `robo vendor list` |
| `installPath` | Default project-relative destination |
| `detectPaths` | Project-relative paths that identify an already-present checkout |
| `sourceUrl` | Optional clone URL; `null` disables fetching |
| `components` | Runtime components the module usually needs |
| `requiredPaths` | Files/directories expected inside the vendor checkout |
| `bootstrapScripts` | Project-owned scripts to run after the source exists |
| `patches` | Project-owned patch files expected by bootstrap scripts |

## Current Limits

`robo vendor` is not a package manager yet. It does not pin revisions, verify checksums, or lock vendor sources. Production hardening should add version pins and audit output before public-source vendor installs are treated as reproducible.
