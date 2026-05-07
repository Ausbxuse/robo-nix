# Why robo-nix

Robot-learning repositories are often harder to reproduce than the ideas they contain.

The pain is not just Python packages. It is the whole stack: compilers, graphics libraries, ROS, simulators, CUDA mismatches, vendor SDKs, host drivers, and fragile setup assumptions that live outside the project.

`robo-nix` exists to make that stack explicit, shareable, and easier to trust.

## The Hidden Tax

Robot learning moves fast. New repositories appear constantly. Simulators change, benchmarks drift, dependencies move, and before you run the first training script you can already be deep in setup work.

That setup burden is still underestimated. Many projects ask users to assemble pages of commands by hand: install a compiler, install Python packages, add CUDA paths, install ROS, patch a simulator, source a vendor SDK, and hope none of it conflicts with the next repo.

Even when the setup eventually works, it is hard to know whether you reproduced the author's environment or just built a fragile local approximation.

## Python Got Better. The System Problem Remained.

`uv` is a major improvement over older Python workflows. It is fast, reliable, and good at the part Python tooling can own.

But robot-learning environments are not only Python environments.

They often need:

- native compilers
- CMake and pkg-config
- OpenGL, EGL, X11, Wayland, and Qt libraries
- FFmpeg and media libraries
- CUDA build tools and host driver compatibility
- ROS distributions
- simulator runtimes
- hardware and vendor SDKs

Python metadata cannot express that whole runtime honestly. That is where `robo-nix` fits.

## The Core Idea

Keep normal Python packaging. Add a reproducible native runtime around it.

`robo-nix` uses a simple boundary:

- `uv` owns Python version selection, Python packages, `.venv` sync, `pyproject.toml`, and `uv.lock`.
- Nix owns the CPython interpreter, native libraries, compilers, simulator tooling, CUDA/graphics/ROS runtime pieces, and shell environment.
- `robo` owns the user workflow and diagnostics.

That gives users a small command surface while keeping the underlying environment declarative and reviewable.

```bash
robo shell
uv sync
python -m pytest
```

The goal is not to hide reality. The goal is to make reality easier to see and easier to reproduce.

## Where It Fits

`robo-nix` is not trying to win every environment category. It is for uv-managed robot-learning projects that need host-integrated native runtime support without asking every user to become a Nix user.

<div class="fit-matrix">

| Tool | Python lock | Native libs | CUDA / graphics | Host diagnostics | Beginner workflow |
| --- | :-: | :-: | :-: | :-: | :-: |
| `robo-nix` | <span class="yes">✓</span> | <span class="yes">✓</span> | <span class="yes">✓</span> | <span class="yes">✓</span> | <span class="yes">✓</span> |
| plain `uv` | <span class="yes">✓</span> | <span class="no">✕</span> | <span class="no">✕</span> | <span class="no">✕</span> | <span class="yes">✓</span> |
| Conda / Pixi | <span class="partial">~</span> | <span class="yes">✓</span> | <span class="partial">~</span> | <span class="partial">~</span> | <span class="partial">~</span> |
| Docker | <span class="partial">~</span> | <span class="yes">✓</span> | <span class="partial">~</span> | <span class="no">✕</span> | <span class="partial">~</span> |
| `uv2nix` | <span class="yes">✓</span> | <span class="yes">✓</span> | <span class="partial">~</span> | <span class="no">✕</span> | <span class="partial">~</span> |
| devenv / raw flakes | <span class="partial">~</span> | <span class="yes">✓</span> | <span class="partial">~</span> | <span class="partial">~</span> | <span class="no">✕</span> |

</div>

<div class="matrix-legend">
  <span class="yes">✓</span> strong fit
  <span class="partial">~</span> possible with tradeoffs
  <span class="no">✕</span> not the tool's job
</div>

- Use `uv` when you only need Python.
- Use Docker when image deployment is the center of the workflow.
- Use Pixi or Conda when your team wants the Conda ecosystem to own the environment.
- Use `uv2nix` when you want Nix to own Python packages too.
- Use devenv when you want a general Nix development shell.
- Use raw flakes when your users are comfortable working directly in Nix.
- Use `robo-nix` when the project should stay normal Python, but native dependencies and diagnostics need to be reproducible for robot-learning work.

## Why Not Just Docker?

Docker is useful, and for some workloads it is the right tool.

For day-to-day robot-learning development, containers can become awkward. GPU access, simulator GUIs, ROS networking, hardware bridges, and vendor SDKs often need tighter host integration than a container story alone can comfortably provide.

`robo-nix` is aimed at reproducible developer environments on real machines. It can complement containers, but it is not only an image-building story.

## Why Not Just uv, Conda, or Poetry?

Those tools solve important Python problems. They do not solve the full robot-learning runtime problem.

`robo-nix` is not trying to replace good Python tooling. It is designed to let Python tooling stay normal while Nix supplies the native layer Python cannot safely own.

## Why Nix?

Nix is powerful because it can describe native software environments precisely.

Nix is also intimidating when every user has to understand it.

`robo-nix` tries to put the complexity in the environment definition and the CLI, not on every downstream user. Most users should not need to learn flakes, derivations, overlays, or Nixpkgs internals just to run a robot-learning repo.

## Why Modularity Matters

Robot-learning projects are too diverse for one monolithic environment.

A teleoperation app may need MuJoCo, OpenGL, Qt, and native C++ build tools. A ROS workspace may need ROS 2 and colcon. A learning repo may need CUDA wheels and a compatible host driver. A hardware project may need local vendor SDK setup.

The scalable model is a library of reusable runtime components that projects compose locally.

That keeps setup explicit without turning the central project into a preset matrix for every possible repository.

## What robo-nix Promises

`robo-nix` is designed to make setup:

- easier to run
- easier to review
- easier to debug
- easier to share
- less dependent on hidden host state

It should also be honest about what it cannot own.

The host still owns GPU drivers, display sockets, hardware devices, and system services. Projects still own their dependency groups, optional extras, private indexes, vendored sources, and bootstrap policy. `robo` should surface those boundaries clearly instead of pretending they disappeared.

That honesty is a feature. A setup tool that hides too much eventually becomes another source of mysterious failures.

## The Practical Win

Environment setup is not a side issue. It directly affects research velocity, reproducibility, onboarding, and collaboration.

Every hidden setup assumption becomes time lost by someone else later.

If robot-learning environments become easier to reproduce and reason about, good ideas become easier to try, extend, and share.
