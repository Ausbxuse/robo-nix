{common}: {
  python-uv = {
    envSpec,
    pkgs,
    ...
  }: {
    packages = [
      pkgs.uv
    ];
    shellInit =
      common.exportVars {
        UV_PYTHON = envSpec.pythonVersion;
        UV_CACHE_DIR = "$WORKSPACE_ROOT/.robo-nix/uv-cache";
      }
      + ''

        if [ -d "$WORKSPACE_ROOT/.venv/bin" ]; then
          export PATH="$WORKSPACE_ROOT/.venv/bin:$PATH"
        fi
      '';
    check = common.mkComponentCheck "python-uv" [];
  };
}
