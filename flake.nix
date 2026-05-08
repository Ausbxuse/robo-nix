{
  description = "Minimal robo-nix rebuild";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {
    self,
    nixpkgs,
    ...
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
  in {
    packages = forAllSystems (system: let
      pkgs = import nixpkgs {inherit system;};
    in {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "robo";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
      };
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
    in pkgs.alejandra);
  };
}
