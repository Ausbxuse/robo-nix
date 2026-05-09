use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

mod bootstrap;
mod error;
mod inference;
mod search;
mod shell_launch;
mod shell_refresh;
mod ui;

use bootstrap::{prepare_project, print_bootstrap_report};
use error::{print_error, write_debug_log, AppError};
use shell_launch::interactive_shell_launch;
use shell_refresh::{runtime_input_state, set_active_shell_env};
use ui::{debug, output_with_tree, status, Config};

const HELP: &str = include_str!("templates/help.txt");

fn main() -> ExitCode {
    let config = ui_config();
    console::set_colors_enabled(config.color);
    console::set_colors_enabled_stderr(config.color);
    match run(env::args_os().skip(1).collect(), config) {
        Ok(code) => code,
        Err(error) => {
            print_error(config, &error);
            if error.should_write_debug_log() {
                match write_debug_log(&error) {
                    Ok(path) => debug(config, &format!("wrote {}", path.display())),
                    Err(err) => debug(
                        config,
                        &format!("failed to write .robo-nix/last-error.log: {err}"),
                    ),
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn ui_config() -> Config {
    let color = env::var_os("NO_COLOR").is_none()
        && (io::stdout().is_terminal() || io::stderr().is_terminal());
    let debug = env::var_os("ROBO_NIX_DEBUG").is_some();
    Config { color, debug }
}

fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_usage();
        return Ok(ExitCode::SUCCESS);
    };
    let command = command
        .to_str()
        .ok_or_else(|| AppError::user("command must be valid UTF-8"))?;

    match command {
        "shell" => shell_command(args.collect(), config),
        "run" => run_command(args.collect(), config),
        "search" => Ok(search::run(args.collect(), config)),
        "__shell-refresh" => Ok(shell_refresh::run(args.collect(), config)),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        "init" => Err(AppError::user("`robo init` has been removed")
            .with_hint("run `robo shell` from a project with .python-version instead.")),
        "check" => Err(AppError::user("`robo check` is not part of this branch")
            .with_hint("run `robo shell`; future correctness checks will use a separate surface.")),
        other => Err(AppError::user(format!("unknown command `{other}`"))),
    }
}

fn print_usage() {
    print!("{HELP}");
}

fn shell_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "shell does not accept arguments; use `robo run` for commands",
        ));
    }
    run_nix_develop(Vec::new(), config)
}

fn run_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if args.is_empty() {
        return Err(AppError::user("run requires a command"));
    }
    run_nix_develop(args, config)
}

fn run_nix_develop(command_args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let report = prepare_project(Path::new("."))?;
    print_bootstrap_report(config, &report);

    let phase = if command_args.is_empty() {
        "shell"
    } else {
        "run"
    };
    preflight_nix_develop(config, phase)?;

    let mut command = Command::new("nix");
    command
        .arg("develop")
        .arg("--accept-flake-config")
        .arg("--command");

    if command_args.is_empty() {
        let launch = interactive_shell_launch().ok_or_else(|| {
            AppError::project("could not determine an interactive shell to launch")
                .with_hint("set ROBO_NIX_SHELL to the shell you want robo to launch.")
        })?;
        status(config, &format!("shell: launching {}", launch.name));
        command.arg(&launch.program).args(&launch.args);
        set_active_shell_env(
            &mut command,
            &workspace_root()?,
            &runtime_input_state(Path::new(".")),
        );
        for (name, value) in launch.env {
            command.env(name, value);
        }
    } else {
        command.args(command_args);
    }

    let status = command.status().map_err(|err| {
        AppError::project(format!("failed to start nix: {err}"))
            .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
    })?;

    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Err(AppError::project(format!("nix develop exited with {status}"))
            .with_hint("review the Nix output above and attach .robo-nix/last-error.log to an issue if this looks like a robo-nix bug."))
    }
}

fn preflight_nix_develop(config: Config, phase: &str) -> Result<(), AppError> {
    let mut command = Command::new("nix");
    command
        .arg("develop")
        .arg("--accept-flake-config")
        .arg("--command")
        .arg("true");
    let output = output_with_tree(
        config,
        &mut command,
        &format!("robo {phase}"),
        &format!("{phase}: evaluating and realizing dev shell"),
    )
    .map_err(|err| {
        AppError::project(format!("failed to start nix: {err}"))
            .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
    })?;

    if output.status.success() {
        return Ok(());
    }

    write_command_output(&output)?;
    Err(AppError::project(format!(
        "nix develop exited with {}",
        output.status
    ))
    .with_hint("review the Nix output above and attach .robo-nix/last-error.log to an issue if this looks like a robo-nix bug."))
}

fn write_command_output(output: &Output) -> Result<(), AppError> {
    io::stdout()
        .write_all(&output.stdout)
        .map_err(|err| AppError::project(format!("failed to write nix stdout: {err}")))?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|err| AppError::project(format!("failed to write nix stderr: {err}")))?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, AppError> {
    env::current_dir()
        .map_err(|err| AppError::project(format!("failed to determine workspace root: {err}")))
}
