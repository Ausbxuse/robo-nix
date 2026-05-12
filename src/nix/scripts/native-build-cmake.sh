real_cmake="@real_cmake@"
robo_cmake_configure=1

for robo_cmake_arg in "$@"; do
  case "$robo_cmake_arg" in
    --build|--install|--open|--find-package|-E|-P|--version|-version|/version|--help|-help|/help)
      robo_cmake_configure=0
      ;;
    --help-*)
      robo_cmake_configure=0
      ;;
  esac
done

if [ "$robo_cmake_configure" != 1 ]; then
  exec "$real_cmake" "$@"
fi

robo_cmake_stdout="$(@coreutils@/bin/mktemp "${TMPDIR:-/tmp}/robo-cmake-stdout.XXXXXX")" || exec "$real_cmake" "$@"
robo_cmake_stderr="$(@coreutils@/bin/mktemp "${TMPDIR:-/tmp}/robo-cmake-stderr.XXXXXX")" || {
  rm -f "$robo_cmake_stdout"
  exec "$real_cmake" "$@"
}
trap 'rm -f "$robo_cmake_stdout" "$robo_cmake_stderr"' EXIT HUP INT TERM

"$real_cmake" "$@" >"$robo_cmake_stdout" 2>"$robo_cmake_stderr"
robo_cmake_status=$?
@coreutils@/bin/cat "$robo_cmake_stdout"
@coreutils@/bin/cat "$robo_cmake_stderr" >&2

if [ "$robo_cmake_status" -ne 0 ] && @gnugrep@/bin/grep -q "Could not find a package configuration file provided by" "$robo_cmake_stderr"; then
  robo_cmake_package="$(
    @gnused@/bin/sed -n 's/.*provided by "\([^"]*\)".*/\1/p' "$robo_cmake_stderr" | @coreutils@/bin/head -n 1
  )"
  if [ -n "$robo_cmake_package" ]; then
    printf '%s\n' "robo-nix hint: CMake could not find package '$robo_cmake_package'." >&2
    printf '%s\n' "robo-nix hint: native-build supplies compiler tools and common native runtime libraries; package-specific CMake config files must come from the project, the uv build environment, or explicit robo.nix additions." >&2
    if [ "$robo_cmake_package" = "Qt6" ]; then
      printf '%s\n' "robo-nix hint: add \"qt6\" to components in robo.nix for Qt6 CMake packages and runtime libraries." >&2
    fi
    printf '%s\n' "robo-nix hint: patch the package build to set ${robo_cmake_package}_DIR or CMAKE_PREFIX_PATH to the prefix containing ${robo_cmake_package}Config.cmake." >&2
  fi
fi

if [ "$robo_cmake_status" -ne 0 ] && @gnugrep@/bin/grep -q "is not a full path to an existing compiler tool" "$robo_cmake_stderr" "$robo_cmake_stdout"; then
  printf '%s\n' "robo-nix hint: CMake is using a cached compiler path that no longer exists." >&2
  printf '%s\n' "robo-nix hint: remove the affected CMake build directory or CMakeCache.txt, then rerun inside the current runtime shell." >&2
fi

exit "$robo_cmake_status"
