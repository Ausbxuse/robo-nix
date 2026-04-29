# Roadmap

Short-term:

- keep `robo init`, `robo check`, `robo sync`, `robo run`, and `robo shell` small and reliable
- keep runtime inference as data under `nix/modules`
- keep Python ownership in `uv`

Later:

- split stable Rust boundaries into `robo-core`, `robo-generate`, and `robo-checks`
- add real templates only after common downstream usage is proven
- package the CLI through native package managers without making Python packaging depend on Nix
