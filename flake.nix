{
  description = "robo-nix rewrite";

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
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
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
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
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
