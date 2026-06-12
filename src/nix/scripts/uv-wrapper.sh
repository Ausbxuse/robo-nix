real_uv="@real_uv@"

if [ -n "${UV_PROJECT_ENVIRONMENT:-}" ]; then
  export VIRTUAL_ENV="$UV_PROJECT_ENVIRONMENT"
  if [ -d "$UV_PROJECT_ENVIRONMENT/bin" ]; then
    case ":$PATH:" in
      *":$UV_PROJECT_ENVIRONMENT/bin:"*) ;;
      *) export PATH="$UV_PROJECT_ENVIRONMENT/bin:$PATH" ;;
    esac
  fi
fi

if [ "${1:-}" = "pip" ] && [ "${2:-}" = "install" ] && [ -n "${UV_PROJECT_ENVIRONMENT:-}" ] && [ -x "$UV_PROJECT_ENVIRONMENT/bin/python" ]; then
  robo_uv_has_target=0
  for robo_uv_arg in "$@"; do
    case "$robo_uv_arg" in
      --)
        break
        ;;
      --python|--python=*|-p|--system|--active|--target|--target=*|--prefix|--prefix=*)
        robo_uv_has_target=1
        ;;
    esac
  done

  if [ "$robo_uv_has_target" = 0 ]; then
    shift 2
    exec "$real_uv" pip install --python "$UV_PROJECT_ENVIRONMENT/bin/python" "$@"
  fi
fi

if [ "${1:-}" = "sync" ]; then
  robo_uv_has_extra_policy=0
  robo_uv_has_group_policy=0
  for robo_uv_arg in "$@"; do
    case "$robo_uv_arg" in
      --)
        break
        ;;
      --extra|--extra=*|--all-extras|--no-extra|--no-extra=*)
        robo_uv_has_extra_policy=1
        ;;
      --group|--group=*|--only-group|--only-group=*|--all-groups|--no-group|--no-group=*|--no-dev|--no-default-groups)
        robo_uv_has_group_policy=1
        ;;
    esac
  done

  if [ "$robo_uv_has_extra_policy" = 0 ] && [ -n "${ROBO_NIX_PYTHON_EXTRAS:-}" ]; then
    robo_uv_extra_args=
    robo_uv_old_ifs="$IFS"
    IFS=:
    for robo_uv_extra in $ROBO_NIX_PYTHON_EXTRAS; do
      if [ -n "$robo_uv_extra" ]; then
        set -- "$@" --extra "$robo_uv_extra"
      fi
    done
    IFS="$robo_uv_old_ifs"
    unset robo_uv_extra robo_uv_old_ifs robo_uv_extra_args
  fi

  if [ "$robo_uv_has_group_policy" = 0 ] && [ -n "${ROBO_NIX_PYTHON_GROUPS_SET:-}" ]; then
    set -- "$@" --no-default-groups
    robo_uv_old_ifs="$IFS"
    IFS=:
    for robo_uv_group in ${ROBO_NIX_PYTHON_GROUPS:-}; do
      if [ -n "$robo_uv_group" ]; then
        set -- "$@" --group "$robo_uv_group"
      fi
    done
    IFS="$robo_uv_old_ifs"
    unset robo_uv_group robo_uv_old_ifs
  fi
fi

exec "$real_uv" "$@"
