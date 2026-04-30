use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use console::measure_text_width;

use crate::{
    error, field, hint, label, output_with_spinner, section, status, Config, LabelKind,
};

use super::bootstrap::run_bootstrap;
use super::cuda_compat::ensure_runtime_cuda_compat;
use super::nix::{
    check_command, command_for_runtime, exit_code, hint_native_cuda_link_failure, nix_command,
    run_status, run_status_after_marker,
};
use super::python::ensure_python_version_files;

pub(crate) fn ensure_project_runtime(config: Config) -> Result<(), ExitCode> {
    if !Path::new("flake.nix").exists() || !Path::new("robo.nix").exists() {
        error(config, "this directory is not initialized for robo-nix.");
        hint(config, "run `robo init .` from the project checkout first.");
        return Err(ExitCode::from(1));
    }
    repair_managed_flake_source(config)?;

    if let Err(message) = check_command("nix") {
        error(config, &message);
        hint(config, "install Nix, then rerun this command.");
        return Err(ExitCode::from(1));
    }

    Ok(())
}

pub(crate) fn run_project_app(mode: Option<&str>, args: Vec<OsString>, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let mut command = nix_command(config);
    command.arg("run").arg(".#default");
    if let Some(mode) = mode {
        command.arg("--").arg(mode);
    }
    command.args(args);
    run_status(&mut command, config)
}

pub(crate) fn run_project_activate(args: Vec<OsString>, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }
    if let Err(code) = prepare_activation_env(config) {
        return code;
    }

    let show_card = args.is_empty();
    let launch = normalize_shell_args(args);
    if show_card {
        print_activation_card(config);
    }

    let mut command = command_for_runtime(config);
    command.env("ROBO_NIX_ACTIVATE", "1");
    if let Ok(current_exe) = env::current_exe() {
        command.env("ROBO_NIX_ROBO_BIN", current_exe);
    }
    for (name, value) in launch.env {
        command.env(name, value);
    }
    run_status(command.arg("develop").args(launch.args), config)
}

fn prepare_activation_env(config: Config) -> Result<(), ExitCode> {
    let mut command = command_for_runtime(config);
    command.arg("develop").arg("-c").arg("true");

    if config.debug {
        let status = run_status(&mut command, config);
        if status == ExitCode::SUCCESS {
            return Ok(());
        }
        return Err(status);
    }

    match output_with_spinner(config, &mut command, "preparing activation environment") {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            error(config, "activation environment failed to build");
            print_captured("stdout", &output.stdout);
            print_captured("stderr", &output.stderr);
            hint_native_cuda_link_failure(config, &output);
            Err(exit_code(output.status.code()))
        }
        Err(err) => {
            error(config, &format!("failed to start activation environment: {err}"));
            Err(ExitCode::from(1))
        }
    }
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- activation {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}

fn print_activation_card(config: Config) {
    let state = RuntimeState::read();
    let system = nix_system_name();
    let workspace = shorten_middle(&home_tilde(&state.workspace), 62);

    let field = |name: &str, value: &str| {
        (
            format!("{name:<7} {value}"),
            format!(
                "{} {}",
                label(config, &format!("{name:<7}"), LabelKind::Hint),
                label(config, value, LabelKind::Status)
            ),
        )
    };
    let field_pair = |left_name: &str, left_value: &str, right_name: &str, right_value: &str| {
        (
            format!("{left_name:<7} {left_value:<8}  {right_name} {right_value}"),
            format!(
                "{} {}  {} {}",
                label(config, &format!("{left_name:<7}"), LabelKind::Hint),
                label(config, &format!("{left_value:<8}"), LabelKind::Status),
                label(config, right_name, LabelKind::Hint),
                label(config, right_value, LabelKind::Status)
            ),
        )
    };
    let action = |command: &str, description: &str| {
        (
            format!("  {command:<9} {description}"),
            format!(
                "  {} {}",
                label(config, &format!("{command:<9}"), LabelKind::Command),
                label(config, description, LabelKind::Hint)
            ),
        )
    };

    let rows = [
        (
            format!("{} runtime", state.env_name),
            label(config, &format!("{} runtime", state.env_name), LabelKind::Status),
        ),
        field_pair("python", &state.python_version, "system", system),
        field("path", &workspace),
        (String::new(), String::new()),
        (
            "commands".to_string(),
            label(config, "commands", LabelKind::Hint),
        ),
        action("uv sync", "sync Python packages from uv.lock"),
        action("exit", "leave this runtime shell"),
    ];
    let row_width = rows
        .iter()
        .map(|(plain, _)| measure_text_width(plain))
        .max()
        .unwrap_or(0);
    let inner_width = row_width + 2;
    let (top_left, horizontal, top_right, vertical, bottom_left, bottom_right) =
        if config.color {
            ("╭", "─", "╮", "│", "╰", "╯")
        } else {
            ("+", "-", "+", "|", "+", "+")
        };

    println!(
        "{}{}{}",
        label(config, top_left, LabelKind::Status),
        label(config, &horizontal.repeat(inner_width), LabelKind::Status),
        label(config, top_right, LabelKind::Status)
    );
    for (plain, rendered) in rows {
        let plain_len = measure_text_width(&plain);
        let padding = " ".repeat(row_width.saturating_sub(plain_len));
        println!(
            "{} {}{} {}",
            label(config, vertical, LabelKind::Status),
            rendered,
            padding,
            label(config, vertical, LabelKind::Status),
        );
    }
    println!(
        "{}{}{}",
        label(config, bottom_left, LabelKind::Status),
        label(config, &horizontal.repeat(inner_width), LabelKind::Status),
        label(config, bottom_right, LabelKind::Status),
    );
}

fn home_tilde(value: &str) -> String {
    let Ok(home) = env::var("HOME") else {
        return value.to_string();
    };
    if value == home {
        return "~".to_string();
    }
    value
        .strip_prefix(&format!("{home}/"))
        .map_or_else(|| value.to_string(), |rest| format!("~/{rest}"))
}

fn shorten_middle(value: &str, max_len: usize) -> String {
    let len = value.chars().count();
    if len <= max_len {
        return value.to_string();
    }

    let keep = max_len.saturating_sub(3);
    let tail: String = value
        .chars()
        .skip(len.saturating_sub(keep))
        .collect();
    format!("...{tail}")
}

pub(crate) fn run_project_status(config: Config) -> ExitCode {
    let state = RuntimeState::read();
    print_runtime_state(config, &state);
    ExitCode::SUCCESS
}

pub(crate) fn run_project_hook(args: Vec<OsString>, config: Config) -> ExitCode {
    let shell = match hook_shell(args.first()) {
        Ok(shell) => shell,
        Err(message) => {
            error(config, &message);
            hint(config, "supported hooks: bash, zsh, fish");
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

pub(crate) fn run_internal_activate_env(config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let mut command = command_for_runtime(config);
    command.arg("print-dev-env").arg(".#default");
    let output = if config.debug {
        match command.output() {
            Ok(output) => output,
            Err(err) => {
                error(config, &format!("failed to load activation environment: {err}"));
                return ExitCode::from(1);
            }
        }
    } else {
        match output_with_spinner(config, &mut command, "loading activation environment") {
            Ok(output) => output,
            Err(err) => {
                error(config, &format!("failed to load activation environment: {err}"));
                return ExitCode::from(1);
            }
        }
    };

    if !output.status.success() {
        error(config, "activation environment failed to load");
        print_captured("stdout", &output.stdout);
        print_captured("stderr", &output.stderr);
        hint_native_cuda_link_failure(config, &output);
        return exit_code(output.status.code());
    }

    let env = match activation_env_exports(&output.stdout) {
        Ok(env) => env,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };
    for (name, value) in env {
        println!("export {name}={}", shell_quote(&value));
    }
    print_activation_env_exports();
    ExitCode::SUCCESS
}

pub(crate) fn run_project_deactivate(config: Config) -> ExitCode {
    let state = RuntimeState::read();
    if state.active {
        section(config, "action");
        action_row(config, "exit", "leave this runtime shell");
        println!(
            "  {}",
            label(
                config,
                "A child process cannot exit its parent shell directly.",
                LabelKind::Hint
            )
        );
    } else {
        println!("{} not activated\n", label(config, "robo:", LabelKind::Status));
        section(config, "status");
        println!("  {}", label(config, "ok", LabelKind::Ok));
    }
    ExitCode::SUCCESS
}

fn print_activation_env_exports() {
    let state = RuntimeState::read();
    println!();
    println!("# robo activation");
    println!("export ROBO_NIX_ACTIVE=1");
    println!("export ROBO_NIX_ENV_NAME={}", shell_quote(&state.env_name));
    println!(
        "export ROBO_NIX_PYTHON_VERSION={}",
        shell_quote(&state.python_version)
    );
    println!("export WORKSPACE_ROOT={}", shell_quote(&state.workspace));
    println!(
        "export ROBO_NIX_PROMPT_PREFIX={}",
        shell_quote(&format!("<{}> ", state.env_name))
    );
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            println!("export PATH={}:\"$PATH\"", shell_quote(&parent.display().to_string()));
        }
    }
}

fn activation_env_exports(script: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(
            "source /dev/stdin >/dev/null; \
             if [ -n \"${shellHook:-}\" ]; then eval \"$shellHook\" >/dev/null; fi; \
             env -0",
        )
        .env("ROBO_NIX_QUIET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to materialize activation environment: {err}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "failed to open activation environment stdin".to_string())?
        .write_all(script)
        .map_err(|err| format!("failed to write activation environment: {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to read activation environment: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("activation shell hook failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(name, _)| should_export_activation_var(name))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect())
}

fn should_export_activation_var(name: &str) -> bool {
    !matches!(
        name,
        "" | "_" | "PWD" | "OLDPWD" | "SHLVL" | "shellHook" | "ROBO_NIX_QUIET"
    ) && !name.starts_with("BASH")
        && name
            .chars()
            .enumerate()
            .all(|(index, ch)| ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit()))
}

pub(crate) fn run_project_command(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "run needs a command.");
        hint(config, "example: robo run pytest tests");
        return ExitCode::from(2);
    }

    if let Err(code) = prepare_uv_runtime(config, "run", true) {
        return code;
    }

    let args = if args.first().is_some_and(|arg| arg == "uv")
        && args.get(1).is_some_and(|arg| arg == "run")
    {
        args.into_iter().skip(2).collect()
    } else {
        args
    };

    run_marked_uv_command(args, config)
}

pub(crate) fn run_internal_exec(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "internal exec needs a command.");
        return ExitCode::from(2);
    }
    let marker = env::var("ROBO_NIX_EXEC_MARKER").unwrap_or_default();
    if !marker.is_empty() {
        let _ = std::io::stdout().write_all(marker.as_bytes());
        let _ = std::io::stderr().write_all(marker.as_bytes());
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }

    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);
    exec_command(command)
}

#[cfg(unix)]
fn exec_command(mut command: Command) -> ExitCode {
    use std::os::unix::process::CommandExt;
    let err = command.exec();
    eprintln!("error: failed to exec command: {err}");
    ExitCode::from(1)
}

#[cfg(not(unix))]
fn exec_command(mut command: Command) -> ExitCode {
    match command.status() {
        Ok(status) => exit_code(status.code()),
        Err(err) => {
            eprintln!("error: failed to exec command: {err}");
            ExitCode::from(1)
        }
    }
}

fn run_marked_uv_command(args: Vec<OsString>, config: Config) -> ExitCode {
    let marker = format!("__ROBO_NIX_COMMAND_STARTED_{}__", std::process::id());
    let Ok(current_exe) = env::current_exe() else {
        return run_status(
            command_for_runtime(config)
                .arg("develop")
                .arg("-c")
                .arg("uv")
                .arg("run")
                .args(args),
            config,
        );
    };

    run_status_after_marker(
        command_for_runtime(config)
            .env("ROBO_NIX_EXEC_MARKER", &marker)
            .arg("develop")
            .arg("-c")
            .arg(current_exe)
            .arg("__exec")
            .arg("uv")
            .arg("run")
            .args(args),
        config,
        &marker,
    )
}

fn prepare_uv_runtime(
    config: Config,
    command_name: &str,
    cuda_strict: bool,
) -> Result<(), ExitCode> {
    ensure_pyproject(config, command_name)?;
    ensure_python_version_files(config)?;
    prepare_runtime(config, cuda_strict)
}

fn prepare_runtime(config: Config, cuda_strict: bool) -> Result<(), ExitCode> {
    ensure_project_runtime(config)?;
    run_bootstrap(config)?;
    ensure_runtime_cuda_compat(config, cuda_strict)
}

struct RuntimeState {
    active: bool,
    env_name: String,
    python_version: String,
    workspace: String,
    shell: Option<String>,
    prompt_prefix: Option<String>,
}

impl RuntimeState {
    fn read() -> Self {
        let active = env::var_os("ROBO_NIX_ACTIVE").is_some();
        let runtime = if Path::new("robo.nix").exists() {
            Some(crate::runtime::read_project_runtime())
        } else {
            None
        };
        let env_name = env::var("ROBO_NIX_ENV_NAME")
            .ok()
            .or_else(|| runtime.as_ref().map(|runtime| runtime.env_name.clone()))
            .unwrap_or_else(|| "unknown".to_string());
        let python_version = env::var("ROBO_NIX_PYTHON_VERSION")
            .ok()
            .or_else(|| runtime.as_ref().map(|runtime| runtime.python_version.clone()))
            .unwrap_or_else(|| "unknown".to_string());
        let workspace = env::var("WORKSPACE_ROOT").unwrap_or_else(|_| {
            env::current_dir().map_or_else(|_| ".".into(), |path| path.display().to_string())
        });

        Self {
            active,
            env_name,
            python_version,
            workspace,
            shell: env::var("SHELL").ok(),
            prompt_prefix: env::var("ROBO_NIX_PROMPT_PREFIX").ok(),
        }
    }
}

fn print_runtime_state(config: Config, state: &RuntimeState) {
    section(config, "runtime");
    field(config, "env", &state.env_name);
    field(
        config,
        "state",
        if state.active { "active" } else { "inactive" },
    );
    field(config, "python", &state.python_version);
    if let Some(shell) = &state.shell {
        field(config, "shell", &shell_name(shell));
    }
    field(config, "workspace", &state.workspace);
    if let Some(prefix) = &state.prompt_prefix {
        field(config, "prompt-prefix", prefix);
    }

    println!();
    if state.active {
        section(config, "actions");
        action_row(
            config,
            "uv sync",
            "sync Python packages from uv.lock",
        );
        action_row(config, "exit", "leave this runtime shell");
    } else {
        section(config, "action");
        action_row(config, "robo activate", "enter the Nix runtime shell");
    }
}

fn action_row(config: Config, command: &str, description: &str) {
    println!(
        "  {:<15} {}",
        label(config, command, LabelKind::Command),
        label(config, description, LabelKind::Hint)
    );
}

fn hook_shell(arg: Option<&OsString>) -> Result<String, String> {
    let shell = match arg {
        Some(shell) => shell.to_string_lossy().into_owned(),
        None => env::var("SHELL")
            .ok()
            .and_then(|shell| {
                Path::new(&shell)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| "robo hook needs a shell name when SHELL is unknown.".to_string())?,
    };

    match shell.as_str() {
        "bash" | "zsh" | "fish" => Ok(shell),
        unknown => Err(format!("unsupported hook shell: {unknown}")),
    }
}

fn print_posix_hook(robo: &Path) {
    let robo = shell_quote(&robo.display().to_string());
    println!(
        r#"__robo_bin={robo}; __robo_save_var() {{ eval "__robo_state=\${{__ROBO_SAVED_${{1}}_STATE+x}}"; if [ -n "$__robo_state" ]; then unset __robo_state; return; fi; eval "__robo_has_value=\${{${{1}}+x}}"; if [ -n "$__robo_has_value" ]; then eval "__ROBO_SAVED_${{1}}_STATE=set"; eval "__ROBO_SAVED_${{1}}=\${{${{1}}}}"; else eval "__ROBO_SAVED_${{1}}_STATE=unset"; fi; unset __robo_state __robo_has_value; }}; __robo_restore_var() {{ eval "__robo_state=\${{__ROBO_SAVED_${{1}}_STATE:-}}"; case "$__robo_state" in set) eval "export $1=\"\${{__ROBO_SAVED_${{1}}}}\"" ;; unset) unset "$1" ;; esac; eval "unset __ROBO_SAVED_${{1}}_STATE __ROBO_SAVED_${{1}}"; unset __robo_state; }}; __robo_prompt_enable() {{ if [ -n "${{ROBO_NIX_PROMPT_PREFIX:-}}" ] && [ -z "${{__ROBO_PROMPT_ACTIVE:-}}" ]; then __ROBO_PROMPT_ACTIVE=1; __ROBO_SAVED_PS1="${{PS1-}}"; PS1="${{ROBO_NIX_PROMPT_PREFIX}}${{PS1-}}"; fi; }}; __robo_prompt_disable() {{ if [ -n "${{__ROBO_PROMPT_ACTIVE:-}}" ]; then PS1="${{__ROBO_SAVED_PS1-}}"; unset __ROBO_PROMPT_ACTIVE __ROBO_SAVED_PS1; fi; }}; robo() {{ case "${{1-}}" in activate) shift; if [ "$#" -eq 0 ]; then __robo_save_var PATH; __robo_save_var LD_LIBRARY_PATH; __robo_save_var LIBRARY_PATH; __robo_save_var CPATH; __robo_save_var CUDA_HOME; __robo_save_var CUDA_PATH; __robo_save_var XDG_DATA_DIRS; __robo_save_var SHELL; __robo_env="$("$__robo_bin" __activate-env)" || return; eval "$__robo_env"; unset __robo_env; __robo_prompt_enable; else "$__robo_bin" activate "$@"; fi ;; deactivate) if [ -n "${{ROBO_NIX_ACTIVE:-}}" ]; then __robo_prompt_disable; __robo_restore_var PATH; __robo_restore_var LD_LIBRARY_PATH; __robo_restore_var LIBRARY_PATH; __robo_restore_var CPATH; __robo_restore_var CUDA_HOME; __robo_restore_var CUDA_PATH; __robo_restore_var XDG_DATA_DIRS; __robo_restore_var SHELL; unset ROBO_NIX_ACTIVE ROBO_NIX_ENV_NAME ROBO_NIX_PYTHON_VERSION ROBO_NIX_PROMPT_PREFIX; hash -r 2>/dev/null || true; else "$__robo_bin" deactivate; fi ;; *) "$__robo_bin" "$@" ;; esac; }}; if [ -n "${{ROBO_NIX_ACTIVE:-}}" ]; then __robo_prompt_enable; fi"#
    );
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

fn shell_name(shell: &str) -> String {
    Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell)
        .to_string()
}

fn nix_system_name() -> &'static str {
    match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "linux") => "x86_64-linux",
        ("aarch64", "linux") => "aarch64-linux",
        ("x86_64", "macos") => "x86_64-darwin",
        ("aarch64", "macos") => "aarch64-darwin",
        _ => env::consts::OS,
    }
}

fn ensure_pyproject(config: Config, command_name: &str) -> Result<(), ExitCode> {
    if Path::new("pyproject.toml").exists() {
        return Ok(());
    }
    error(config, &format!("{command_name} needs pyproject.toml."));
    hint(config, "run `robo init .` or create pyproject.toml for uv.");
    Err(ExitCode::from(1))
}

fn repair_managed_flake_source(config: Config) -> Result<(), ExitCode> {
    let flake_path = Path::new("flake.nix");
    let flake = match fs::read_to_string(flake_path) {
        Ok(flake) => flake,
        Err(err) => {
            error(config, &format!("failed to read flake.nix: {err}"));
            return Err(ExitCode::from(1));
        }
    };
    if !flake.contains("mkProjectFlakeFromManifest") {
        error(config, "flake.nix does not look generated by robo.");
        hint(
            config,
            "run `robo init . --force` only if you want robo to replace this flake.",
        );
        return Err(ExitCode::from(1));
    }
    let source_url = match env::var("ROBO_NIX_DEFAULT_SOURCE_URL") {
        Ok(source_url) => source_url,
        Err(_) => {
            if flake.contains("github:ausbxuse/robo-nix") {
                error(
                    config,
                    "flake.nix points at github:ausbxuse/robo-nix, but this robo install has no packaged source URL.",
                );
                hint(
                    config,
                    "run `robo init . --robo-nix-url path:/path/to/robo-nix` to repair this project.",
                );
                return Err(ExitCode::from(1));
            }
            return Ok(());
        }
    };
    let Some(current_source_url) = managed_robo_nix_url(&flake) else {
        return Ok(());
    };
    if current_source_url == source_url {
        return Ok(());
    }
    let repaired = flake.replace(
        &format!("inputs.robo-nix.url = \"{current_source_url}\";"),
        &format!("inputs.robo-nix.url = \"{source_url}\";"),
    );
    if let Err(err) = fs::write(flake_path, repaired) {
        error(config, &format!("failed to repair flake.nix: {err}"));
        return Err(ExitCode::from(1));
    }
    status(config, &format!("repaired flake.nix to use {source_url}"));
    hint(
        config,
        "packaged robo-nix source avoids copying large local checkout paths during activation.",
    );
    Ok(())
}

fn managed_robo_nix_url(flake: &str) -> Option<String> {
    flake.lines().find_map(|line| {
        line.trim()
            .strip_prefix("inputs.robo-nix.url = \"")
            .and_then(|rest| rest.strip_suffix("\";"))
            .map(ToOwned::to_owned)
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ShellLaunch {
    args: Vec<OsString>,
    env: Vec<(String, OsString)>,
}

impl ShellLaunch {
    fn args(args: Vec<OsString>) -> Self {
        Self { args, env: vec![] }
    }
}

fn normalize_shell_args(mut args: Vec<OsString>) -> ShellLaunch {
    if args.is_empty() {
        return default_interactive_shell_args();
    }

    if args.len() != 2 || args[0] != "-c" {
        return ShellLaunch::args(args);
    }

    if !args[1].to_string_lossy().chars().any(char::is_whitespace) {
        return ShellLaunch::args(args);
    }

    let mut normalized = Vec::with_capacity(4);
    normalized.push(OsString::from("-c"));
    normalized.push(OsString::from("bash"));
    normalized.push(OsString::from("-lc"));
    normalized.push(args.swap_remove(1));
    ShellLaunch::args(normalized)
}

fn default_interactive_shell_args() -> ShellLaunch {
    let Some(shell) = default_interactive_shell() else {
        return ShellLaunch::args(vec![]);
    };
    shell_args_for(shell.to_string_lossy().as_ref())
}

fn default_interactive_shell() -> Option<PathBuf> {
    if let Some(shell) = env::var_os("ROBO_NIX_SHELL").map(PathBuf::from) {
        return Some(shell);
    }
    if let Some(shell) = parent_interactive_shell() {
        return Some(shell);
    }
    if let Some(shell) = env::var_os("SHELL").map(PathBuf::from) {
        if is_nix_bash(&shell) {
            return login_shell().filter(|login| login.is_file()).or(Some(shell));
        }
        return Some(shell);
    }
    login_shell()
}

fn parent_interactive_shell() -> Option<PathBuf> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    let ppid = after_comm.split_whitespace().nth(1)?;
    let shell = fs::read_link(format!("/proc/{ppid}/exe")).ok()?;
    shell.is_file().then_some(shell)
}

fn is_nix_bash(shell: &Path) -> bool {
    shell.to_string_lossy().contains("/nix/store/")
        && shell.file_name().is_some_and(|name| name == "bash")
}

fn login_shell() -> Option<PathBuf> {
    let user = env::var("USER").ok()?;
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != user {
            return None;
        }
        fields.nth(5).map(PathBuf::from)
    })
}

fn shell_args_for(shell: &str) -> ShellLaunch {
    if shell.is_empty() || !Path::new(shell).is_file() {
        return ShellLaunch::args(vec![]);
    }

    ShellLaunch {
        args: vec![
            OsString::from("-c"),
            OsString::from(shell),
            OsString::from("-i"),
        ],
        env: vec![
            ("SHELL".to_string(), OsString::from(shell)),
            (
                "ROBO_NIX_ACTIVATION_SHELL".to_string(),
                OsString::from(shell),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn shell_command_with_single_quoted_string_uses_shell() {
        let args = vec![OsString::from("-c"), OsString::from("python test.py")];
        let normalized = normalize_shell_args(args);
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "bash", "-lc", "python test.py"]);
    }

    #[test]
    fn shell_command_without_args_uses_user_shell() {
        let normalized = shell_args_for("/bin/sh");
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "/bin/sh", "-i"]);
        assert_eq!(
            normalized.env,
            vec![
                ("SHELL".to_string(), OsString::from("/bin/sh")),
                (
                    "ROBO_NIX_ACTIVATION_SHELL".to_string(),
                    OsString::from("/bin/sh")
                )
            ]
        );
    }

    #[test]
    fn shell_command_keeps_split_argv_intact() {
        let args = vec![
            OsString::from("-c"),
            OsString::from("python"),
            OsString::from("test.py"),
        ];
        let normalized = normalize_shell_args(args.clone());
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "python", "test.py"]);
        assert_eq!(normalized, ShellLaunch::args(args));
    }
}
