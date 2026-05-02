{lib}: let
  common = import ./common.nix {inherit lib;};
in
  lib.foldl' lib.recursiveUpdate {} [
    (import ./devshell.nix {inherit common;})
    (import ./python.nix {inherit common lib;})
    (import ./graphics.nix {inherit common;})
    (import ./cuda.nix {inherit common;})
    (import ./ros.nix {inherit common;})
  ]
