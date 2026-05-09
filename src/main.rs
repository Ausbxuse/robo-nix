use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

mod bootstrap;
mod error;
mod inference;
mod nix_env;
mod search;
mod shell_launch;
mod shell_refresh;
mod ui;

use bootstrap::{prepare_project, print_bootstrap_report};
use error::{print_error, write_debug_log, AppError};
use nix_env::{apply_env, dev_environment};
use shell_launch::interactive_shell_launch;
use shell_refresh::{runtime_input_state, set_active_shell_env};
use ui::{debug, help_row, list_item, section, status, Config};

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
        print_usage(config);
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
            print_usage(config);
            Ok(ExitCode::SUCCESS)
        }
        "init" => Err(AppError::user("`robo init` has been removed")
            .with_hint("run `robo shell` from a project with .python-version instead.")),
        "check" => Err(AppError::user("`robo check` is not part of this branch")
            .with_hint("run `robo shell`; future correctness checks will use a separate surface.")),
        other => Err(AppError::user(format!("unknown command `{other}`"))),
    }
}

fn print_usage(config: Config) {
    section(config, "usage");
    help_row(config, "robo shell", "open an interactive runtime shell");
    help_row(
        config,
        "robo run <command>",
        "run a command inside the prepared runtime",
    );
    help_row(
        config,
        "robo search <library>",
        "find a Nix runtime library package",
    );

    println!();
    section(config, "project setup");
    list_item(config, ".python-version is required.");
    list_item(config, "pyproject.toml is managed by uv/project policy.");
    list_item(
        config,
        "robo shell creates missing robo runtime files on first use.",
    );

    println!();
    section(config, "runtime lookup");
    help_row(
        config,
        "robo search libassimp.so",
        "find packages for missing shared libraries",
    );
}

fn shell_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "shell does not accept arguments; use `robo run` for commands",
        ));
    }
    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        return Err(nested_shell_error());
    }
    run_nix_develop(Vec::new(), config)
}

fn nested_shell_error() -> AppError {
    AppError::user("already inside a robo shell")
        .with_hint("exit the current shell before running `robo shell` again.")
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
    let dev_env = dev_environment(config, phase)?;
    let mut command = if command_args.is_empty() {
        shell_launch_command(config, &dev_env)?
    } else {
        run_launch_command(command_args, &dev_env)?
    };

    let status = command.status().map_err(|err| {
        AppError::project(format!("failed to launch {phase} command: {err}"))
            .with_hint("review the command and make sure it exists in the robo shell environment.")
    })?;

    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Err(
            AppError::project(format!("{phase} command exited with {status}")).with_hint(
                "the runtime was prepared successfully; inspect the command output above.",
            ),
        )
    }
}

fn shell_launch_command(config: Config, dev_env: &[(String, String)]) -> Result<Command, AppError> {
    let launch = interactive_shell_launch().ok_or_else(|| {
        AppError::project("could not determine an interactive shell to launch")
            .with_hint("set ROBO_NIX_SHELL to the shell you want robo to launch.")
    })?;
    status(config, &format!("shell: launching {}", launch.name));

    let mut command = Command::new(&launch.program);
    command.args(&launch.args);
    apply_env(&mut command, dev_env);
    for (name, value) in launch.env {
        command.env(name, value);
    }
    set_active_shell_env(
        &mut command,
        &workspace_root()?,
        &runtime_input_state(Path::new(".")),
    );
    Ok(command)
}

fn run_launch_command(
    command_args: Vec<OsString>,
    dev_env: &[(String, String)],
) -> Result<Command, AppError> {
    let mut command_args = command_args.into_iter();
    let program = command_args
        .next()
        .ok_or_else(|| AppError::user("run requires a command"))?;
    let mut command = Command::new(program);
    command.args(command_args);
    apply_env(&mut command, dev_env);
    Ok(command)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_shell_error_names_the_boundary() {
        let error = nested_shell_error();

        assert!(error.message().contains("already inside a robo shell"));
    }
}
