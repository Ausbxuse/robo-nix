use std::collections::{hash_map::DefaultHasher, HashMap};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use console::measure_text_width;

use crate::shell::{
    SUPPORTED_INTERACTIVE_SHELLS, requested_shell_name, supports_interactive_shell,
};
use crate::{Config, LabelKind, UiSpinner, error, field, hint, label, section, status};

use super::bootstrap::run_bootstrap;
use super::cuda_compat::ensure_runtime_cuda_compat;
use super::nix::{
    add_runtime_source_override, check_command, command_for_runtime, exit_code,
    hint_native_cuda_link_failure, nix_command, run_status,
};
use super::python::ensure_python_version_files;

const HOOK_STATE_VARS: &[&str] = &[
    "PATH",
    "LD_LIBRARY_PATH",
    "LIBRARY_PATH",
    "CPATH",
    "CMAKE_PREFIX_PATH",
    "CUDA_HOME",
    "CUDA_PATH",
    "MUJOCO_GL",
    "NIX_CFLAGS_COMPILE",
    "NIX_LDFLAGS",
    "ROBO_NIX_PYTHON",
    "ROBO_NIX_PYTHON_MAJOR_MINOR",
    "XDG_DATA_DIRS",
    "SHELL",
    "UV_CACHE_DIR",
    "UV_PROJECT_ENVIRONMENT",
    "UV_PYTHON",
    "UV_PYTHON_DOWNLOADS",
    "VIRTUAL_ENV",
];

const SHELL_ENV_CAPTURE_SCRIPT: &str = "source /dev/stdin >/dev/null; \
     if [ -n \"${shellHook:-}\" ]; then eval \"$shellHook\" >/dev/null; fi; \
     env -0";

pub(crate) fn ensure_project_runtime(config: Config) -> Result<(), ExitCode> {
    if !Path::new("flake.nix").exists() || !Path::new("robo.nix").exists() {
        error(config, "this directory is not initialized for robo-nix.");
        hint(config, "run `robo up` from the project checkout first.");
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
    command.arg("run");
    add_runtime_source_override(&mut command);
    command.arg(".#default");
    if let Some(mode) = mode {
        command.arg("--").arg(mode);
    }
    command.args(args);
    run_status(&mut command, config)
}

pub(crate) fn run_project_up(
    target: PathBuf,
    yes: bool,
    open_shell: bool,
    config: Config,
) -> ExitCode {
    let implicit_yes = yes || open_shell;

    if !target.exists() {
        if !implicit_yes && !confirm_create_dir(config, &target) {
            hint(config, "rerun with `robo up --yes <dir>` to create it non-interactively.");
            return ExitCode::from(1);
        }
        if let Err(err) = fs::create_dir_all(&target) {
            error(
                config,
                &format!("failed to create project directory {}: {err}", target.display()),
            );
            return ExitCode::from(1);
        }
    }

    if let Err(err) = env::set_current_dir(&target) {
        error(
            config,
            &format!("failed to enter project directory {}: {err}", target.display()),
        );
        return ExitCode::from(1);
    }

    let initialized = Path::new("flake.nix").exists() && Path::new("robo.nix").exists();
    if !initialized {
        if open_shell {
            status(config, "up: initializing runtime files");
        } else if !implicit_yes && !confirm_up_init(config) {
            hint(config, "rerun with `robo up --yes` to initialize non-interactively.");
            return ExitCode::from(1);
        }
        let code = crate::init::run_quiet(
            crate::init::InitArgs::generated(PathBuf::from("."), false, false),
            config,
        );
        if code != ExitCode::SUCCESS {
            return code;
        }
    }

    status(config, "up: checking runtime files");
    if let Err(code) = prepare_uv_runtime(config, "up", false) {
        return code;
    }
    let mut progress = ShellProgress::new(config, "up: caching runtime shell");
    let shell_script = match load_shell_env_script(config, Some(&progress)) {
        Ok(script) => script,
        Err(code) => {
            progress.finish();
            return code;
        }
    };
    let env = match materialize_shell_env(&shell_script, config, Some(&progress)) {
        Ok(env) => env,
        Err(code) => {
            progress.finish();
            return code;
        }
    };
    progress.finish();

    write_shell_env_cache_if_possible(&env, config);

    if open_shell {
        println!("robo is ready for this project.");
        println!("Entering the runtime shell...");
        return run_project_shell(vec![], config);
    }
    println!("robo is ready for this project.\n");
    println!("Python packages are not synced yet.");
    println!("Run the uv command documented by this project.");
    println!(
        "Default: `{}`",
        label(config, "uv sync", LabelKind::Command)
    );
    println!();
    println!("Enter the runtime shell:");
    action_row(config, "robo shell", "open an interactive runtime shell");
    ExitCode::SUCCESS
}

fn confirm_create_dir(config: Config, target: &Path) -> bool {
    if !io::stdin().is_terminal() {
        error(
            config,
            &format!(
                "project directory {} does not exist and stdin is not interactive.",
                target.display()
            ),
        );
        return false;
    }
    section(config, "setup");
    field(config, "directory", &target.display().to_string());
    print!(
        "{} ",
        label(
            config,
            "Create this project directory and continue? [Y/n]",
            LabelKind::Hint,
        )
    );
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes")
}

fn confirm_up_init(config: Config) -> bool {
    if !io::stdin().is_terminal() {
        error(config, "no robo.nix found and stdin is not interactive.");
        return false;
    }
    println!("No robo runtime files were found in this project.\n");
    print!(
        "{} ",
        label(
            config,
            "Create robo.nix, flake.nix, and .python-version? [Y/n]",
            LabelKind::Hint,
        )
    );
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes")
}

pub(crate) fn run_project_shell(args: Vec<OsString>, config: Config) -> ExitCode {
    let state = RuntimeState::read();
    if state.active {
        print_already_active(config, &state);
        return ExitCode::SUCCESS;
    }

    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let show_card = args.is_empty();
    let launch = normalize_shell_args(args);
    if launch.args.is_empty() {
        error(config, "could not determine an interactive shell to launch.");
        hint(
            config,
            "set ROBO_NIX_SHELL to the shell you want robo to launch.",
        );
        return ExitCode::from(1);
    }
    let mut progress = ShellProgress::new(config, "shell: loading cached runtime shell");
    let env = match load_cached_or_refresh_shell_env(config, Some(&progress)) {
        Ok(env) => env,
        Err(code) => {
            progress.finish();
            return code;
        }
    };
    progress.set("shell: launching shell");
    progress.finish();
    if show_card {
        print_shell_card(config, &launch);
    }

    run_shell(launch, env, config)
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- shell {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}

fn print_shell_card(config: Config, launch: &ShellLaunch) {
    let state = RuntimeState::read();
    let system = nix_system_name();
    let workspace = shorten_middle(&home_tilde(&state.workspace), 62);
    let shell = shell_launch_label(launch);

    let rows = [
        (
            format!("{} runtime", state.env_name),
            label(
                config,
                &format!("{} runtime", state.env_name),
                LabelKind::Status,
            ),
        ),
        card_field_pair(config, "python", &state.python_version, "system", system),
        card_field(config, "path", &workspace),
        card_field(config, "shell", &shell),
        (String::new(), String::new()),
        (
            "commands".to_string(),
            label(config, "commands", LabelKind::Hint),
        ),
        card_action(config, "uv sync", "sync Python packages from uv.lock"),
        card_action(config, "exit", "leave this runtime shell"),
    ];
    let row_width = rows
        .iter()
        .map(|(plain, _)| measure_text_width(plain))
        .max()
        .unwrap_or(0);
    let inner_width = row_width + 2;
    let (top_left, horizontal, top_right, vertical, bottom_left, bottom_right) = if config.color {
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

fn shell_launch_label(launch: &ShellLaunch) -> String {
    let mut args = launch.args.iter();
    if args.next().is_some_and(|arg| arg == "-c") {
        let Some(shell) = args.next() else {
            return "unknown".to_string();
        };
        let shell_name = Path::new(shell)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| shell.to_str().unwrap_or("unknown"));
        shell_name.to_string()
    } else {
        launch
            .args
            .first()
            .and_then(|arg| Path::new(arg).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}

fn card_field(config: Config, name: &str, value: &str) -> (String, String) {
    (
        format!("{name:<7} {value}"),
        format!(
            "{} {}",
            label(config, &format!("{name:<7}"), LabelKind::Hint),
            label(config, value, LabelKind::Status)
        ),
    )
}

fn card_field_pair(
    config: Config,
    left_name: &str,
    left_value: &str,
    right_name: &str,
    right_value: &str,
) -> (String, String) {
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
}

fn card_action(config: Config, command: &str, description: &str) -> (String, String) {
    (
        format!("  {command:<9} {description}"),
        format!(
            "  {} {}",
            label(config, &format!("{command:<9}"), LabelKind::Command),
            label(config, description, LabelKind::Hint)
        ),
    )
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
    let tail: String = value.chars().skip(len.saturating_sub(keep)).collect();
    format!("...{tail}")
}

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

pub(crate) fn run_internal_shell_env(config: Config) -> ExitCode {
    let mut progress = ShellProgress::new(config, "shell: checking runtime files");
    if let Err(code) = ensure_project_runtime(config) {
        progress.finish();
        return code;
    }

    let env = match load_cached_or_refresh_shell_env(config, Some(&progress)) {
        Ok(env) => env,
        Err(code) => {
            progress.finish();
            return code;
        }
    };
    progress.finish();
    print_exports(&env);
    ExitCode::SUCCESS
}

fn load_shell_env(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    let script = load_shell_env_script(config, progress)?;
    materialize_shell_env(&script, config, progress)
}

fn load_shell_env_script(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<u8>, ExitCode> {
    let mut command = command_for_runtime(config);
    command.arg("print-dev-env");
    add_runtime_source_override(&mut command);
    command.arg(".#default");
    if let Some(progress) = progress {
        progress.set("shell: evaluating and realizing dev shell");
    }
    if progress.is_none() {
        status(config, "shell: evaluating and realizing dev shell");
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => {
            error(
                config,
                &format!("failed to load shell environment: {err}"),
            );
            return Err(ExitCode::from(1));
        }
    };

    if !output.status.success() {
        error(config, "shell environment failed to load");
        print_captured("stdout", &output.stdout);
        print_captured("stderr", &output.stderr);
        hint_native_cuda_link_failure(config, &output);
        return Err(exit_code(output.status.code()));
    }

    Ok(output.stdout)
}

fn materialize_shell_env(
    script: &[u8],
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    if let Some(progress) = progress {
        progress.set("shell: capturing shell environment");
    }
    let mut env = match shell_env_exports(script) {
        Ok(env) => env,
        Err(message) => {
            error(config, &message);
            return Err(ExitCode::from(1));
        }
    };
    if let Some(progress) = progress {
        progress.set("shell: applying runtime exports");
    }
    append_shell_state(&mut env);
    Ok(env)
}

fn load_cached_or_refresh_shell_env(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    match read_shell_env_cache() {
        Ok(Some(env)) => {
            if let Some(progress) = progress {
                progress.set("shell: using cached runtime shell");
            } else {
                status(config, "shell: using cached runtime shell");
            }
            Ok(env)
        }
        Ok(None) => refresh_shell_env_cache(config, progress),
        Err(message) => {
            if config.debug {
                hint(config, &format!("ignoring stale shell cache: {message}"));
            }
            refresh_shell_env_cache(config, progress)
        }
    }
}

fn refresh_shell_env_cache(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    let env = load_shell_env(config, progress)?;
    write_shell_env_cache_if_possible(&env, config);
    Ok(env)
}

fn write_shell_env_cache_if_possible(env: &[(String, String)], config: Config) {
    if let Err(message) = write_shell_env_cache(env) {
        if config.debug {
            hint(config, &format!("failed to write shell cache: {message}"));
        }
    }
}

struct ShellProgress {
    config: Config,
    spinner: Option<UiSpinner>,
}

impl ShellProgress {
    fn new(config: Config, message: &str) -> Self {
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self {
                config,
                spinner: None,
            };
        }

        Self {
            config,
            spinner: Some(UiSpinner::new(config, message)),
        }
    }

    fn set(&self, message: &str) {
        if let Some(spinner) = &self.spinner {
            spinner.set_message(message);
        } else {
            status(self.config, message);
        }
    }

    fn finish(&mut self) {
        if let Some(spinner) = &mut self.spinner {
            spinner.finish();
        }
    }
}

fn run_shell(
    launch: ShellLaunch,
    env: Vec<(String, String)>,
    config: Config,
) -> ExitCode {
    let mut command = match command_from_launch_args(launch.args) {
        Ok(command) => command,
        Err(message) => {
            error(config, &message);
            hint(
                config,
                "set ROBO_NIX_SHELL to the shell you want robo to launch.",
            );
            return ExitCode::from(1);
        }
    };
    apply_env(&mut command, &env);
    for (name, value) in launch.env {
        command.env(name, value);
    }
    exec_command(command)
}

fn apply_env(command: &mut Command, env: &[(String, String)]) {
    for (name, value) in env {
        command.env(name, value);
    }
}

fn command_from_launch_args(args: Vec<OsString>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Err("could not determine an interactive shell to launch.".to_string());
    };

    let (program, args): (OsString, Vec<_>) = if first == "-c" {
        let Some(program) = args.next() else {
            return Err("shell command is missing a program after -c.".to_string());
        };
        (program, args.collect())
    } else {
        (first, args.collect())
    };

    let mut command = Command::new(program);
    command.args(args);
    Ok(command)
}

fn print_exports(env: &[(String, String)]) {
    for (name, value) in env {
        println!("export {name}={}", shell_quote(&value));
    }
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
        section(config, "status");
        println!("  {}", label(config, "inactive", LabelKind::Hint));
        println!();
        section(config, "action");
        action_row(config, "robo shell", "enter the Nix runtime shell");
    }
    ExitCode::SUCCESS
}

fn append_shell_state(envs: &mut Vec<(String, String)>) {
    let state = RuntimeState::read();
    set_shell_env(envs, "ROBO_NIX_ACTIVE", "1".to_string());
    set_shell_env(envs, "ROBO_NIX_ENV_NAME", state.env_name.clone());
    set_shell_env(
        envs,
        "ROBO_NIX_PYTHON_VERSION",
        state.python_version.clone(),
    );
    set_shell_env(envs, "WORKSPACE_ROOT", state.workspace.clone());
    set_shell_env(
        envs,
        "ROBO_NIX_PROMPT_PREFIX",
        format!("<{}> ", state.env_name),
    );

    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let parent = parent.display().to_string();
            let base = shell_env_value(envs, "PATH")
                .cloned()
                .or_else(|| env::var("PATH").ok())
                .unwrap_or_default();
            set_shell_env(envs, "PATH", format!("{parent}:{base}"));
        }
    }
}

fn shell_env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a String> {
    envs.iter()
        .rev()
        .find_map(|(candidate, value)| (candidate == name).then_some(value))
}

fn set_shell_env(envs: &mut Vec<(String, String)>, name: &str, value: String) {
    if let Some((_, existing)) = envs.iter_mut().find(|(candidate, _)| candidate == name) {
        *existing = value;
    } else {
        envs.push((name.to_string(), value));
    }
}

fn shell_env_cache_dir() -> PathBuf {
    PathBuf::from(".robo-nix")
}

fn shell_env_cache_path() -> PathBuf {
    shell_env_cache_dir().join("shell-env")
}

fn shell_env_cache_key_path() -> PathBuf {
    shell_env_cache_dir().join("shell-env.key")
}

fn read_shell_env_cache() -> Result<Option<Vec<(String, String)>>, String> {
    let key_path = shell_env_cache_key_path();
    let env_path = shell_env_cache_path();
    if !key_path.exists() || !env_path.exists() {
        return Ok(None);
    }

    let expected = shell_env_cache_key();
    let actual = fs::read_to_string(&key_path)
        .map_err(|err| format!("failed to read {}: {err}", key_path.display()))?;
    if actual.trim() != expected {
        return Ok(None);
    }

    let bytes = fs::read(&env_path)
        .map_err(|err| format!("failed to read {}: {err}", env_path.display()))?;
    parse_cached_shell_env(&bytes).map(Some)
}

fn write_shell_env_cache(env: &[(String, String)]) -> Result<(), String> {
    let dir = shell_env_cache_dir();
    fs::create_dir_all(&dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    fs::write(shell_env_cache_path(), serialize_shell_env(env))
        .map_err(|err| format!("failed to write shell env cache: {err}"))?;
    fs::write(shell_env_cache_key_path(), format!("{}\n", shell_env_cache_key()))
        .map_err(|err| format!("failed to write shell env cache key: {err}"))?;
    Ok(())
}

fn serialize_shell_env(env: &[(String, String)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, value) in env {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn parse_cached_shell_env(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let chunks: Vec<_> = bytes.split(|byte| *byte == 0).collect();
    let mut entries = chunks.as_slice();
    if entries.last().is_some_and(|entry| entry.is_empty()) {
        entries = &entries[..entries.len() - 1];
    }
    if entries.len() % 2 != 0 {
        return Err("shell env cache is truncated".to_string());
    }

    entries
        .chunks(2)
        .map(|pair| {
            let name = String::from_utf8(pair[0].to_vec())
                .map_err(|_| "shell env cache contains an invalid variable name".to_string())?;
            let value = String::from_utf8(pair[1].to_vec())
                .map_err(|_| "shell env cache contains an invalid variable value".to_string())?;
            Ok((name, value))
        })
        .collect()
}

fn shell_env_cache_key() -> String {
    let mut hasher = DefaultHasher::new();
    nix_system_name().hash(&mut hasher);
    env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .hash(&mut hasher);
    env::var("ROBO_NIX_DEFAULT_SOURCE_URL").ok().hash(&mut hasher);
    env::var("ROBO_NIX_RUNTIME_SOURCE_URL").ok().hash(&mut hasher);
    for name in [
        "ROBO_NIX_WORKSPACE",
        "ROBO_NIX_LIBCUDA_PATH",
        "ROBO_NIX_CUDA_ROOT",
        "UV_PROJECT_ENVIRONMENT",
    ] {
        name.hash(&mut hasher);
        env::var(name).ok().hash(&mut hasher);
    }

    for path in [
        "flake.nix",
        "flake.lock",
        "robo.nix",
        ".python-version",
        "pyproject.toml",
        "uv.lock",
    ] {
        path.hash(&mut hasher);
        match fs::read(path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => 0_u8.hash(&mut hasher),
        }
    }
    hash_venv_cmake_prefixes(&mut hasher);

    format!("{:016x}", hasher.finish())
}

fn hash_venv_cmake_prefixes<H: Hasher>(hasher: &mut H) {
    let pyvenv = Path::new(".venv/pyvenv.cfg");
    pyvenv.hash(hasher);
    match fs::read(pyvenv) {
        Ok(bytes) => bytes.hash(hasher),
        Err(_) => 0_u8.hash(hasher),
    }

    let mut prefixes = Vec::new();
    let Ok(python_dirs) = fs::read_dir(".venv/lib") else {
        prefixes.hash(hasher);
        return;
    };
    for python_dir in python_dirs.flatten() {
        let site_packages = python_dir.path().join("site-packages");
        if !site_packages.is_dir() {
            continue;
        }
        if site_packages.join("share/cmake").is_dir() {
            prefixes.push(site_packages.display().to_string());
        }
        let Ok(entries) = fs::read_dir(&site_packages) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("share/cmake").is_dir() {
                prefixes.push(path.display().to_string());
            }
        }
    }
    prefixes.sort();
    prefixes.hash(hasher);
}

fn shell_env_exports(script: &[u8]) -> Result<Vec<(String, String)>, String> {
    let baseline: HashMap<_, _> = env::vars().collect();
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(SHELL_ENV_CAPTURE_SCRIPT)
        .env("ROBO_NIX_QUIET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to materialize shell environment: {err}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "failed to open shell environment stdin".to_string())?
        .write_all(script)
        .map_err(|err| format!("failed to write shell environment: {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to read shell environment: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("shell hook failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(name, value)| should_export_shell_var(name, value, &baseline))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect())
}

fn should_export_shell_var(name: &str, value: &str, baseline: &HashMap<String, String>) -> bool {
    !is_shell_export_blocked(name)
        && is_shell_identifier(name)
        && baseline.get(name).is_none_or(|baseline| baseline != value)
}

fn is_shell_export_blocked(name: &str) -> bool {
    matches!(
        name,
        "" | "_" | "PWD" | "OLDPWD" | "SHLVL" | "SHELL" | "shellHook" | "ROBO_NIX_QUIET"
    ) || name.starts_with("BASH")
}

fn is_shell_identifier(name: &str) -> bool {
    name.chars().enumerate().all(|(index, ch)| {
        ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
    })
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

    let env = match load_cached_or_refresh_shell_env(config, None) {
        Ok(env) => env,
        Err(code) => return code,
    };

    run_uv_command(args, env, config)
}

pub(crate) fn run_internal_exec(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "internal exec needs a command.");
        return ExitCode::from(2);
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

fn run_uv_command(args: Vec<OsString>, env: Vec<(String, String)>, config: Config) -> ExitCode {
    let mut command = Command::new("uv");
    command.arg("run").args(args);
    apply_env(&mut command, &env);
    run_status(&mut command, config)
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
}

impl RuntimeState {
    fn read() -> Self {
        let active = env::var_os("ROBO_NIX_ACTIVE").is_some();
        let runtime = if !active && Path::new("robo.nix").exists() {
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
            .or_else(|| {
                runtime
                    .as_ref()
                    .map(|runtime| runtime.python_version.clone())
            })
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
        }
    }
}

fn print_already_active(config: Config, state: &RuntimeState) {
    section(config, "status");
    println!("  {}", label(config, "already active", LabelKind::Ok));
    println!();
    section(config, "runtime");
    field(config, "env", &state.env_name);
    field(config, "python", &state.python_version);
    if let Some(shell) = &state.shell {
        field(config, "shell", &shell_name(shell));
    }
    field(config, "workspace", &state.workspace);
    println!();
    section(config, "actions");
    action_row(config, "uv sync", "sync Python packages from uv.lock");
    action_row(config, "exit", "leave this runtime shell");
}

fn action_row(config: Config, command: &str, description: &str) {
    println!(
        "  {:<15} {}",
        label(config, command, LabelKind::Command),
        label(config, description, LabelKind::Hint)
    );
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
    let mut flake = match fs::read_to_string(flake_path) {
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
    let source_url = env::var("ROBO_NIX_DEFAULT_SOURCE_URL")
        .unwrap_or_else(|_| "github:ausbxuse/robo-nix".to_string());
    let Some(current_source_url) = managed_robo_nix_url(&flake) else {
        return Ok(());
    };

    let mut changed = false;
    let mut added_binary_caches = false;
    if current_source_url != source_url && is_nonshareable_store_source(&current_source_url) {
        flake = flake.replace(
            &format!("inputs.robo-nix.url = \"{current_source_url}\";"),
            &format!("inputs.robo-nix.url = \"{source_url}\";"),
        );
        changed = true;
    }

    if !flake.contains("nixpkgs-python.cachix.org") {
        let Some(repaired) = add_generated_flake_nix_config(&flake) else {
            return Ok(());
        };
        flake = repaired;
        changed = true;
        added_binary_caches = true;
    }

    if !changed {
        return Ok(());
    }
    if let Err(err) = fs::write(flake_path, flake) {
        error(config, &format!("failed to repair flake.nix: {err}"));
        return Err(ExitCode::from(1));
    }
    if current_source_url != source_url && is_nonshareable_store_source(&current_source_url) {
        status(config, &format!("repaired flake.nix to use {source_url}"));
        hint(config, "portable robo-nix source URLs keep generated projects shareable across hosts.");
    }
    if added_binary_caches {
        status(config, "repaired flake.nix to use robo-nix binary caches");
        hint(
            config,
            "the nixpkgs-python cache avoids compiling Python interpreters when substitutes are available.",
        );
    }
    Ok(())
}

fn add_generated_flake_nix_config(flake: &str) -> Option<String> {
    if flake.contains("nixConfig =") {
        return None;
    }

    let nix_config = r#"  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = [
      "https://nixpkgs-python.cachix.org"
      "https://ros.cachix.org"
    ];
    extra-trusted-public-keys = [
      "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
      "ros.cachix.org-1:dSyZxI8geDCJrwgvCOHDoAfOm5sV1wCPjBkKL+38Rvo="
    ];
  };

"#;

    flake
        .strip_prefix("{\n")
        .map(|rest| format!("{{\n{nix_config}{rest}"))
}

fn managed_robo_nix_url(flake: &str) -> Option<String> {
    flake.lines().find_map(|line| {
        line.trim()
            .strip_prefix("inputs.robo-nix.url = \"")
            .and_then(|rest| rest.strip_suffix("\";"))
            .map(ToOwned::to_owned)
    })
}

fn is_nonshareable_store_source(source_url: &str) -> bool {
    source_url.starts_with("path:/nix/store/")
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
    select_default_interactive_shell(
        env::var_os("ROBO_NIX_SHELL").map(PathBuf::from),
        env::var_os("SHELL").map(PathBuf::from),
        parent_interactive_shell(),
        login_shell(),
        find_shell_in_path,
    )
}

fn select_default_interactive_shell(
    robo_nix_shell: Option<PathBuf>,
    shell_env: Option<PathBuf>,
    parent_shell: Option<PathBuf>,
    login_shell: Option<PathBuf>,
    find_in_path: impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let resolve = |shell| resolve_shell_path_with(shell, &find_in_path);

    if let Some(shell) = robo_nix_shell.and_then(resolve) {
        return Some(shell);
    }

    let shell_env = shell_env.and_then(resolve);
    if let Some(shell) = shell_env.as_deref() {
        if is_nix_bash(shell) {
            return login_shell
                .and_then(resolve)
                .filter(|shell| !is_generic_sh(shell))
                .or_else(|| parent_shell.clone().filter(|shell| !is_generic_sh(shell)))
                .or_else(|| shell_env.clone());
        }
        if !is_generic_sh(shell) {
            return shell_env;
        }
    }

    if let Some(shell) = login_shell
        .and_then(resolve)
        .filter(|shell| !is_generic_sh(shell))
    {
        return Some(shell);
    }

    if let Some(shell) = parent_shell.filter(|shell| !is_generic_sh(shell)) {
        return Some(shell);
    }

    find_in_path("zsh")
        .or_else(|| find_in_path("bash"))
        .or_else(|| find_in_path("fish"))
        .or(shell_env)
        .or_else(|| find_in_path("sh"))
}

fn resolve_shell_path(shell: PathBuf) -> Option<PathBuf> {
    resolve_shell_path_with(shell, &find_shell_in_path)
}

fn resolve_shell_path_with(
    shell: PathBuf,
    find_in_path: &impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if shell.is_file() {
        return Some(shell);
    }
    shell
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(find_in_path)
}

fn find_shell_in_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        return None;
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
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

fn is_generic_sh(shell: &Path) -> bool {
    shell
        .file_name()
        .is_some_and(|name| name == "sh" || name == "dash")
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
    let Some(shell) = resolve_shell_path(PathBuf::from(shell)) else {
        return ShellLaunch::args(vec![]);
    };
    let shell_name = shell
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let interactive_args = clean_interactive_shell_args(shell_name);

    ShellLaunch {
        args: std::iter::once(OsString::from("-c"))
            .chain(std::iter::once(shell.clone().into_os_string()))
            .chain(interactive_args.into_iter().map(OsString::from))
            .collect(),
        env: vec![
            ("SHELL".to_string(), shell.clone().into_os_string()),
        ],
    }
}

fn clean_interactive_shell_args(shell_name: &str) -> Vec<&'static str> {
    match shell_name {
        "bash" | "zsh" | "fish" => vec!["-i"],
        _ => vec!["-i"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

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
            vec![("SHELL".to_string(), OsString::from("/bin/sh"))]
        );
    }

    #[test]
    fn default_shell_loads_user_startup_files() {
        assert_eq!(clean_interactive_shell_args("zsh"), vec!["-i"]);
        assert_eq!(clean_interactive_shell_args("bash"), vec!["-i"]);
        assert_eq!(clean_interactive_shell_args("fish"), vec!["-i"]);
    }

    #[test]
    fn shell_env_cache_round_trips_values() {
        let env = vec![
            ("PATH".to_string(), "/nix/store/bin:/usr/bin".to_string()),
            ("ROBO_NIX_ACTIVE".to_string(), "1".to_string()),
            ("CMAKE_PREFIX_PATH".to_string(), "/tmp/pkg/share/cmake".to_string()),
        ];

        assert_eq!(
            parse_cached_shell_env(&serialize_shell_env(&env)).expect("cache should parse"),
            env
        );
    }

    #[test]
    fn shell_env_cache_rejects_truncated_entries() {
        let bytes = b"PATH\0/bin\0ROBO_NIX_ACTIVE";

        assert_eq!(
            parse_cached_shell_env(bytes).expect_err("truncated cache should fail"),
            "shell env cache is truncated"
        );
    }

    #[test]
    fn shell_env_exports_skip_unchanged_parent_values() {
        let baseline = HashMap::from([
            ("EXPECTED_SHELL".to_string(), "/tmp/first".to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
        ]);

        assert!(!should_export_shell_var(
            "EXPECTED_SHELL",
            "/tmp/first",
            &baseline
        ));
        assert!(should_export_shell_var("PATH", "/nix/bin:/usr/bin", &baseline));
        assert!(!should_export_shell_var("SHELL", "/nix/store/bash", &baseline));
    }

    #[test]
    fn only_nix_store_path_sources_are_repaired() {
        assert!(is_nonshareable_store_source(
            "path:/nix/store/example-source"
        ));
        assert!(!is_nonshareable_store_source(
            "path:/home/user/src/robo-nix"
        ));
        assert!(!is_nonshareable_store_source("github:ausbxuse/robo-nix"));
    }

    #[test]
    fn generated_flake_cache_config_is_inserted_once() {
        let flake = r#"{
  inputs.robo-nix.url = "github:ausbxuse/robo-nix";

  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}"#;

        let repaired = add_generated_flake_nix_config(flake).unwrap();
        assert!(repaired.contains("nixpkgs-python.cachix.org"));
        assert!(repaired.contains("inputs.robo-nix.url"));
        assert!(add_generated_flake_nix_config(&repaired).is_none());
    }

    #[test]
    fn shell_card_labels_shell_name() {
        let launch = ShellLaunch::args(vec![
            OsString::from("-c"),
            OsString::from("/usr/bin/zsh"),
            OsString::from("-i"),
        ]);

        assert_eq!(shell_launch_label(&launch), "zsh");
    }

    #[test]
    fn generic_sh_is_not_treated_as_user_default_shell() {
        assert!(is_generic_sh(Path::new("/bin/sh")));
        assert!(is_generic_sh(Path::new("/usr/bin/dash")));
        assert!(!is_generic_sh(Path::new("/bin/zsh")));
        assert!(!is_generic_sh(Path::new("/bin/bash")));
    }

    #[test]
    fn generic_shell_env_defers_to_parent_zsh() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("/bin/sh")),
            Some(PathBuf::from("/usr/bin/zsh")),
            Some(PathBuf::from("/bin/sh")),
            |_| None,
        );

        assert_eq!(selected, Some(PathBuf::from("/usr/bin/zsh")));
    }

    #[test]
    fn nix_bash_with_generic_login_shell_defers_to_parent_zsh() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("/nix/store/abc-bash-5.3/bin/bash")),
            Some(PathBuf::from("/usr/bin/zsh")),
            Some(PathBuf::from("/bin/sh")),
            |name| {
                (name == "bash").then(|| PathBuf::from("/nix/store/abc-bash-5.3/bin/bash"))
            },
        );

        assert_eq!(selected, Some(PathBuf::from("/usr/bin/zsh")));
    }

    #[test]
    fn shell_command_uses_program_after_develop_command_flag() {
        let command = command_from_launch_args(vec![
            OsString::from("-c"),
            OsString::from("/bin/sh"),
            OsString::from("-i"),
        ])
        .expect("shell command should parse");

        assert_eq!(command.get_program(), "/bin/sh");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-i"]
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
