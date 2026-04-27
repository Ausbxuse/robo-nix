{lib}: let
  common = import ./common.nix {inherit lib;};
in
  lib.foldl' lib.recursiveUpdate {} [
    (import ./core.nix {inherit common;})
    (import ./ros.nix {inherit common;})
    (import ./sim.nix {inherit common;})
  ]
