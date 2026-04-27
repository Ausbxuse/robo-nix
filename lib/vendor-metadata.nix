let
  dir = ./vendor-modules;
  entries = builtins.readDir dir;
  files = builtins.filter (name: builtins.match ".*\\.nix" name != null) (builtins.attrNames entries);
in
  builtins.foldl' (acc: file: acc // import (dir + "/${file}")) {} files
