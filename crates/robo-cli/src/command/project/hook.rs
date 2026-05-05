use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::shell::{SUPPORTED_INTERACTIVE_SHELLS, requested_shell_name, supports_interactive_shell};
use crate::{Config, error, hint};

const HOOK_STATE_VARS: &[&str] = &[
    "PATH",
    "LD_LIBRARY_PATH",
    "LIBRARY_PATH",
    "CPATH",
    "CMAKE_PREFIX_PATH",
    "CUDA_HOME",
    "CUDA_PATH",
    "MUJOCO_GL",
    "ROBO_NIX_MUJOCO_GL_DEFAULT",
    "NIX_CFLAGS_COMPILE",
    "NIX_LDFLAGS",
    "ROBO_NIX_PYTHON",
    "ROBO_NIX_PYTHON_MAJOR_MINOR",
    "ROBO_NIX_HOST_GRAPHICS_AUTO",
    "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO",
    "ROBO_NIX_LIBCUDA_PATH",
    "ROBO_NIX_HOST_LIBCUDA_AUTO",
    "TRITON_LIBCUDA_PATH",
    "XDG_DATA_DIRS",
    "SHELL",
    "UV_CACHE_DIR",
    "UV_PROJECT_ENVIRONMENT",
    "UV_PYTHON",
    "UV_PYTHON_DOWNLOADS",
    "VIRTUAL_ENV",
];

pub(crate) fn run_project_hook(args: Vec<OsString>, config: Config) -> ExitCode {
    let shell = match hook_shell(args.first()) {
        Ok(shell) => shell,
        Err(message) => {
            error(config, &message);
            hint(
                config,
                &format!("supported hooks: {SUPPORTED_INTERACTIVE_SHELLS}"),
            );
            return ExitCode::from(2);
        }
    };
    let robo = env::current_exe()
        .ok()
        .and_then(|path| path.is_file().then_some(path))
        .unwrap_or_else(|| PathBuf::from("robo"));

    match shell.as_str() {
        "bash" | "zsh" => print_posix_hook(&robo),
        "fish" => print_fish_hook(&robo),
        _ => unreachable!(),
    }
    ExitCode::SUCCESS
}

fn hook_shell(arg: Option<&OsString>) -> Result<String, String> {
    let shell = requested_shell_name(arg, "robo hook")?;
    if supports_interactive_shell(&shell) {
        Ok(shell)
    } else {
        Err(format!("unsupported hook shell: {shell}"))
    }
}

fn print_posix_hook(robo: &Path) {
    println!("{}", posix_hook_text(robo));
}

fn posix_hook_text(robo: &Path) -> String {
    let save_vars = posix_hook_var_calls("__robo_save_var");
    let restore_vars = posix_hook_var_calls("__robo_restore_var");
    [
        format!("__robo_bin={}", shell_quote(&robo.display().to_string())),
        posix_save_var_function(),
        posix_restore_var_function(),
        posix_prompt_enable_function(),
        posix_prompt_disable_function(),
        posix_robo_function(&save_vars, &restore_vars),
        r#"if [ -n "${ROBO_NIX_ACTIVE:-}" ]; then __robo_prompt_enable; fi"#.to_string(),
    ]
    .join("; ")
}

fn posix_save_var_function() -> String {
    posix_function(
        "__robo_save_var",
        &[
            r#"eval "__robo_state=\${__ROBO_SAVED_${1}_STATE+x}""#,
            r#"if [ -n "$__robo_state" ]; then unset __robo_state; return; fi"#,
            r#"eval "__robo_has_value=\${${1}+x}""#,
            r#"if [ -n "$__robo_has_value" ]; then eval "__ROBO_SAVED_${1}_STATE=set"; eval "__ROBO_SAVED_${1}=\${${1}}"; else eval "__ROBO_SAVED_${1}_STATE=unset"; fi"#,
            r#"unset __robo_state __robo_has_value"#,
        ],
    )
}

fn posix_restore_var_function() -> String {
    posix_function(
        "__robo_restore_var",
        &[
            r#"eval "__robo_state=\${__ROBO_SAVED_${1}_STATE:-}""#,
            r#"case "$__robo_state" in set) eval "export $1=\"\${__ROBO_SAVED_${1}}\"" ;; unset) unset "$1" ;; esac"#,
            r#"eval "unset __ROBO_SAVED_${1}_STATE __ROBO_SAVED_${1}""#,
            r#"unset __robo_state"#,
        ],
    )
}

fn posix_prompt_enable_function() -> String {
    posix_function(
        "__robo_prompt_enable",
        &[
            r#"if [ -n "${ROBO_NIX_PROMPT_PREFIX:-}" ] && [ -z "${__ROBO_PROMPT_ACTIVE:-}" ]; then __ROBO_PROMPT_ACTIVE=1; __ROBO_SAVED_PS1="${PS1-}"; PS1="${ROBO_NIX_PROMPT_PREFIX}${PS1-}"; fi"#,
        ],
    )
}

fn posix_prompt_disable_function() -> String {
    posix_function(
        "__robo_prompt_disable",
        &[
            r#"if [ -n "${__ROBO_PROMPT_ACTIVE:-}" ]; then PS1="${__ROBO_SAVED_PS1-}"; unset __ROBO_PROMPT_ACTIVE __ROBO_SAVED_PS1; fi"#,
        ],
    )
}

fn posix_robo_function(save_vars: &str, restore_vars: &str) -> String {
    let shell = format!(
        r#"shell) shift; if [ -n "${{ROBO_NIX_ACTIVE:-}}" ]; then "$__robo_bin" status; return; fi; if [ "$#" -eq 0 ]; then {save_vars}; __robo_env="$("$__robo_bin" __shell-env)" || return; eval "$__robo_env"; unset __robo_env; __robo_prompt_enable; else "$__robo_bin" shell "$@"; fi ;;"#
    );
    let deactivate = format!(
        r#"deactivate) if [ -n "${{ROBO_NIX_ACTIVE:-}}" ]; then __robo_prompt_disable; {restore_vars}; unset ROBO_NIX_ACTIVE ROBO_NIX_ENV_NAME ROBO_NIX_PYTHON_VERSION ROBO_NIX_PROMPT_PREFIX; hash -r 2>/dev/null || true; else "$__robo_bin" deactivate; fi ;;"#
    );
    format!(
        r#"robo() {{ case "${{1-}}" in {shell} {deactivate} *) "$__robo_bin" "$@" ;; esac; }}"#
    )
}

fn posix_function(name: &str, body: &[&str]) -> String {
    format!("{name}() {{ {}; }}", body.join("; "))
}

fn posix_hook_var_calls(function: &str) -> String {
    HOOK_STATE_VARS
        .iter()
        .map(|name| format!("{function} {name}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn print_fish_hook(robo: &Path) {
    let robo = fish_quote(&robo.display().to_string());
    println!(
        r#"
set -gx __robo_bin {robo}

function robo
    command $__robo_bin $argv
end

if test -n "$ROBO_NIX_ACTIVE"; and test -n "$ROBO_NIX_PROMPT_PREFIX"
    if functions -q fish_prompt; and not functions -q __robo_fish_prompt_orig
        functions -c fish_prompt __robo_fish_prompt_orig
    end

    function fish_prompt --description 'robo prompt prefix'
        printf '%s' "$ROBO_NIX_PROMPT_PREFIX"
        functions -q __robo_fish_prompt_orig; and __robo_fish_prompt_orig
    end
end
"#
    );
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn fish_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_hook_stays_single_line_for_unquoted_eval() {
        let hook = posix_hook_text(Path::new("/bin/robo"));

        assert!(!hook.contains('\n'));
        assert!(hook.contains("__shell-env"));
        assert!(hook.contains(r#"if [ -n "${ROBO_NIX_ACTIVE:-}" ]"#));
        assert!(hook.contains("__robo_save_var PATH"));
        assert!(hook.contains("__robo_save_var MUJOCO_GL"));
        assert!(hook.contains("__robo_restore_var SHELL"));
    }
}
