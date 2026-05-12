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

exec "$real_uv" "$@"
