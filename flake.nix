{
  description = "robo-nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-python = {
      url = "github:cachix/nixpkgs-python";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    nixpkgs-python,
    ...
  }: let
    lib = nixpkgs.lib;
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = lib.genAttrs systems;
    sourceRoot = toString ./.;
    sourceRelativePath = path:
      lib.removePrefix "${sourceRoot}/" (toString path);
    ignoredSourcePath = path: let
      relativePath = sourceRelativePath path;
    in
      relativePath == "target"
      || lib.hasPrefix "target/" relativePath
      || relativePath == ".robo-nix"
      || lib.hasPrefix ".robo-nix/" relativePath
      || relativePath == "docs/node_modules"
      || lib.hasPrefix "docs/node_modules/" relativePath
      || relativePath == "docs/.vitepress/cache"
      || lib.hasPrefix "docs/.vitepress/cache/" relativePath
      || relativePath == "docs/.vitepress/dist"
      || lib.hasPrefix "docs/.vitepress/dist/" relativePath;
    roboSource = lib.cleanSourceWith {
      src = ./.;
      filter = path: _type: !(ignoredSourcePath path);
    };
    projectLib = import ./src/nix/project-flake.nix {
      inherit nixpkgs nixpkgs-python;
    };
  in {
    lib = {
      inherit (projectLib) mkProjectFlake mkProjectFlakeFromManifest;
    };

    packages = forAllSystems (system: let
      pkgs = import nixpkgs {inherit system;};
      robo = pkgs.rustPlatform.buildRustPackage {
        pname = "robo";
        version = "0.1.0";
        src = roboSource;
        cargoLock.lockFile = ./Cargo.lock;
        ROBO_NIX_BUILD_SOURCE_URL = "path:${roboSource}";
        meta = {
          mainProgram = "robo";
          license = pkgs.lib.licenses.gpl3Only;
        };
      };
    in {
      default = robo;
      inherit robo;
    });

    checks = forAllSystems (system: {
      default = self.packages.${system}.default;
    });

    devShells = forAllSystems (system: let
      pkgs = import nixpkgs {inherit system;};
    in {
      default = pkgs.mkShell {
        packages = [
          pkgs.cargo
          pkgs.nix
          pkgs.rustc
          pkgs.rustfmt
        ];
      };
    });

    formatter = forAllSystems (system: let
      pkgs = import nixpkgs {inherit system;};
    in
      pkgs.alejandra);
  };
}
