use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::{Config, LabelKind, error, field, hint, label, section, status};

use super::bootstrap::run_bootstrap;
use super::cuda_compat::ensure_runtime_cuda_compat;
use super::nix::{
    add_runtime_source_override, check_command, nix_command, run_status,
};
use super::python::ensure_python_version_files;

mod flake_repair;
mod host_bridge;
mod hook;
mod shell_card;
mod shell_env;
mod shell_launch;

use flake_repair::repair_managed_flake_source;
pub(crate) use hook::run_project_hook;
use shell_card::print_shell_card;
use shell_env::{
    ShellProgress, load_cached_or_refresh_shell_env, load_shell_env_script, materialize_shell_env,
    write_shell_env_cache_if_possible,
};
use shell_launch::{
    ShellLaunch, command_from_launch_args, normalize_shell_args,
};

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
    force: bool,
    open_shell: bool,
    config: Config,
) -> ExitCode {
    let implicit_yes = yes || force || open_shell;

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
            crate::init::InitArgs::generated(PathBuf::from("."), false, force),
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
    if let Some(path) = host_bridge::auto_host_cuda_driver_path(&env) {
        status(
            config,
            &format!("up: detected NVIDIA CUDA driver at {path}"),
        );
    }
    if let Some(manifests) = host_bridge::auto_host_graphics_manifests(&env) {
        status(
            config,
            &format!("up: detected NVIDIA graphics manifests: {manifests}"),
        );
    }
    if let Some(dirs) = host_bridge::auto_host_graphics_library_dirs(&env) {
        status(
            config,
            &format!("up: detected NVIDIA graphics libraries at {dirs}"),
        );
    }

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

pub(crate) fn run_project_command(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "run needs a command.");
        hint(config, "example: robo run pytest tests");
        return ExitCode::from(2);
    }

    if let Err(code) = ensure_uv_project_runtime(config, "run") {
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
    ensure_uv_project_runtime(config, command_name)?;
    prepare_runtime(config, cuda_strict)
}

fn ensure_uv_project_runtime(config: Config, command_name: &str) -> Result<(), ExitCode> {
    ensure_pyproject(config, command_name)?;
    ensure_python_version_files(config)?;
    ensure_project_runtime(config)
}

fn prepare_runtime(config: Config, cuda_strict: bool) -> Result<(), ExitCode> {
    ensure_project_runtime(config)?;
    run_bootstrap(config)?;
    ensure_runtime_cuda_compat(config, cuda_strict)
}

pub(super) struct RuntimeState {
    active: bool,
    pub(super) env_name: String,
    pub(super) python_version: String,
    pub(super) workspace: String,
    shell: Option<String>,
}

impl RuntimeState {
    pub(super) fn read() -> Self {
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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shell_name(shell: &str) -> String {
    Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell)
        .to_string()
}

pub(super) fn nix_system_name() -> &'static str {
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
