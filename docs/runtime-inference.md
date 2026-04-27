# Runtime Inference

`robo-nix` exists to fill the native runtime gap around uv-managed Python projects.

`pyproject.toml` and `uv.lock` describe Python packages. They usually do not describe host libraries such as OpenGL, FFmpeg, Qt, CUDA drivers, Linux headers, ROS tools, or simulator runtimes. `robo init` therefore probes the project and writes a small `robo.nix` that makes those runtime assumptions explicit.

## Current Probe Sources

`robo init` currently uses conservative local signals:

- `pyproject.toml` dependencies and `requires-python`
- existing `third_party/*` directories
- existing `scripts/apply_vendor_patches.sh`
- existing `scripts/bootstrap_*.sh`
- simple keywords inside bootstrap scripts, such as Qt, XRoboto, and Linux header references
- simple `source_checkout_ready "$dir" file...` checks in bootstrap scripts

The probe is intentionally small. It should prefer understandable output over clever guesses.

## Package Rules

Package, workspace, and bootstrap-script rules live in [`lib/runtime-inference.nix`](../lib/runtime-inference.nix:1). The packaged CLI receives those rules as data in its generated manifest, so expanding common inference coverage does not require changing Rust code.

| Python signal | Runtime components |
| --- | --- |
| `mujoco`, `dm-control`, `gymnasium-robotics` | `mujoco`, `x11-gl` |
| `glfw`, `pyglet` | `x11-gl` |
| `opencv-python`, `opencv-contrib-python` | `x11-gl`, `media` |
| `av`, `pyav`, `imageio-ffmpeg`, `ffmpeg-python`, `decord` | `media` |
| `lerobot` | `media`, `x11-gl` |
| `pyside6`, `pyqt6`, `pyqt5` | `qt6`, `x11-gl` |
| `torch`, `torchvision`, `jax`, `jaxlib`, `flash-attn`, `triton` | `native-build` |

`matplotlib` alone is not treated as a GUI signal because many projects use it headlessly for saved figures, notebooks, reports, and CI. If a project uses `plt.show()`, add a GUI binding such as `pyqt6`; `robo doctor` will then probe the Qt and `QtAgg` runtime path after `robo sync`.

Manual flags still work:

```bash
robo init . --with qt6,linux-headers
```

Disable probing when you need a fully manual manifest:

```bash
robo init . --no-probe --profile minimal
```

## Workspace And Script Rules

`lib/runtime-inference.nix` also controls:

- the default profile used by `robo init`
- workspace directory roots to inspect, such as `third_party`
- directory-name keywords that imply runtime components
- script roots and filename prefixes to inspect
- text markers for scripts that should not be sourced automatically
- simple script text markers that imply runtime components

## Failure Modes To Track

- False positives: a package may have an optional feature that does not need the inferred native library.
- False negatives: a package may load a native library through a plugin or dynamic import that is not obvious from `pyproject.toml`.
- CPU/GPU ambiguity: `torch` and `jax` do not by themselves prove whether CUDA is required.
- Headless vs GUI ambiguity: OpenCV headless wheels and GUI wheels need different runtime surfaces.
- Vendor scripts: project bootstrap scripts can hide native requirements behind shell logic.
- Script parsing: `robo init` only recognizes simple shell patterns; complex dynamic paths still need manual `--required-file` or `--required-dir`.
- Git flakes: new generated files in a git repo must be registered with git before Nix can see them.
- Host drivers: Nix can provide user-space CUDA libraries, but it cannot install the NVIDIA kernel driver.

When inference is uncertain, `doctor` should explain the suspicion and the next probe to run rather than pretending the environment is known.

## Provenance And Contract Output

Generated `robo.nix` files include `schemaVersion = 1` and a `provenance` block. New projects record `componentReasons`, where each component has:

- `name`
- `source`, such as `profile`, `pyproject inference`, `workspace inference`, or `manual config`
- `reason`

Use the human explanation path while editing:

```bash
robo doctor --why
```

Use the JSON forms for CI snapshots, audits, or review diffs:

```bash
robo doctor --why --json
robo contract --json
```

Older generated projects without `schemaVersion` or `componentReasons` still work, but `doctor` will warn that the schema version is missing and provenance will fall back to coarser inference notes. Regenerate with `robo init . --force` only after reviewing local edits to `robo.nix`.
