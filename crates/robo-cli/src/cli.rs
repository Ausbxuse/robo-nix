use anstyle::AnsiColor;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::command::{
    run_internal_activate_env, run_internal_exec, run_project_activate, run_project_app,
    run_project_command, run_project_deactivate, run_project_hook, run_project_status,
};
use crate::{check, contract, cuda, error, init, Config};

#[derive(Parser)]
#[command(
    name = "robo",
    about = "robo-nix project runtime helper",
    long_about = "Make pyproject.toml + uv work with the native robotics libraries they need, without requiring users to learn flakes first.",
    styles = clap_styles(),
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Show subprocess commands and raw bootstrap output"
    )]
    debug: bool,

    #[arg(long, global = true, help = "Disable ANSI colors")]
    no_color: bool,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    #[command(about = "Initialize robo-nix runtime files")]
    Init(init::InitArgs),

    #[command(about = "Activate the project runtime environment")]
    Activate(PassthroughArgs),

    #[command(about = "Show current runtime activation status")]
    Status,

    #[command(about = "Show how to leave the active runtime shell")]
    Deactivate,

    #[command(about = "Print shell integration for prompt-aware activation")]
    Hook(PassthroughArgs),

    #[command(about = "Run project bootstrap scripts")]
    Bootstrap(PassthroughArgs),

    #[command(about = "Check the current project runtime")]
    Check(check::CheckArgs),

    #[command(about = "Print the resolved runtime contract")]
    Contract(contract::ContractArgs),

    #[command(
        name = "dry-run",
        about = "Validate bootstrap without entering a shell"
    )]
    DryRun(PassthroughArgs),

    #[command(about = "Run a Python command with uv inside the project runtime")]
    Run(PassthroughArgs),

    #[command(hide = true)]
    Completion(CompletionArgs),

    #[command(name = "cuda-check", hide = true)]
    CudaCheck,

    #[command(name = "__exec", hide = true)]
    InternalExec(PassthroughArgs),

    #[command(name = "__activate-env", hide = true)]
    InternalActivateEnv,

    #[command(name = "__cuda-driver-probe", hide = true)]
    InternalCudaDriverProbe,

    #[command(about = "Show help")]
    Help,
}

#[derive(Args)]
struct PassthroughArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

#[derive(Args)]
struct CompletionArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

pub(crate) fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };

    let debug = cli.debug || env::var_os("ROBO_NIX_DEBUG").is_some();
    let color = !cli.no_color
        && env::var_os("NO_COLOR").is_none()
        && (std::io::stdout().is_terminal() || std::io::stderr().is_terminal());
    let config = Config {color, debug};
    console::set_colors_enabled(config.color);
    console::set_colors_enabled_stderr(config.color);

    match cli.command {
        Some(CliCommand::Init(args)) => init::run(args, config),
        Some(CliCommand::Activate(args)) => run_project_activate(args.args, config),
        Some(CliCommand::Status) => run_project_status(config),
        Some(CliCommand::Deactivate) => run_project_deactivate(config),
        Some(CliCommand::Hook(args)) => run_project_hook(args.args, config),
        Some(CliCommand::Bootstrap(args)) => run_project_app(None, args.args, config),
        Some(CliCommand::Check(args)) => check::run(args, config),
        Some(CliCommand::Contract(args)) => contract::run(args, config),
        Some(CliCommand::DryRun(args)) => run_project_app(Some("--dry-run"), args.args, config),
        Some(CliCommand::Run(args)) => run_project_command(args.args, config),
        Some(CliCommand::Completion(args)) => print_completions(args.args, config),
        Some(CliCommand::CudaCheck) => cuda::check(config),
        Some(CliCommand::InternalExec(args)) => run_internal_exec(args.args, config),
        Some(CliCommand::InternalActivateEnv) => run_internal_activate_env(config),
        Some(CliCommand::InternalCudaDriverProbe) => cuda::driver_probe(config),
        Some(CliCommand::Help) | None => print_help(config),
    }
}

fn clap_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(AnsiColor::Cyan.on_default().bold())
        .usage(AnsiColor::Cyan.on_default().bold())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Yellow.on_default())
        .error(AnsiColor::Red.on_default().bold())
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default())
}

fn print_help(config: Config) -> ExitCode {
    let mut command = Cli::command();
    if let Err(err) = command.print_help() {
        error(config, &format!("failed to print help: {err}"));
        return ExitCode::from(1);
    }
    println!();
    ExitCode::SUCCESS
}

fn print_completions(args: Vec<OsString>, config: Config) -> ExitCode {
    let shell = match completion_shell(args.first()) {
        Ok(shell) => shell,
        Err(message) => return completion_error(config, &message),
    };

    let mut command = Cli::command();
    generate(shell, &mut command, "robo", &mut std::io::stdout());
    ExitCode::SUCCESS
}

fn completion_shell(shell: Option<&OsString>) -> Result<Shell, String> {
    let shell = match shell {
        Some(shell) => shell.to_string_lossy().into_owned(),
        None => match env::var("SHELL") {
            Ok(shell) if shell.ends_with("bash") => "bash".to_string(),
            Ok(shell) if shell.ends_with("zsh") => "zsh".to_string(),
            Ok(shell) if shell.ends_with("fish") => "fish".to_string(),
            _ => return Err("robo completion needs a shell name when SHELL is unknown.".to_string()),
        },
    };

    match shell.as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        unknown => Err(format!("unsupported completion shell: {unknown}")),
    }
}

fn completion_error(config: Config, message: &str) -> ExitCode {
    error(config, message);
    crate::hint(config, "supported shells: bash, zsh, fish");
    ExitCode::from(2)
}
