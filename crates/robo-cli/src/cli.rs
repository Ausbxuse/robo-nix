use anstyle::AnsiColor;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::command::{
    run_internal_exec, run_internal_shell_env, run_project_app, run_project_command,
    run_project_deactivate, run_project_hook, run_project_shell, run_project_status,
    run_project_up,
};
use crate::shell::{SUPPORTED_INTERACTIVE_SHELLS, requested_shell_name};
use crate::{Config, LabelKind, check, contract, cuda, error, init, label};

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

    #[command(about = "Prepare the project runtime")]
    Up(UpArgs),

    #[command(about = "Open the project runtime shell")]
    Shell(PassthroughArgs),

    #[command(about = "Show current runtime shell status")]
    Status,

    #[command(about = "Show how to leave the active runtime shell")]
    Deactivate,

    #[command(
        about = "Print shell integration for prompt-aware runtime shells",
        after_help = "Examples:
  eval \"$(robo hook)\"       install the hook for the current shell
  eval \"$(robo hook zsh)\"   print the zsh hook explicitly

After installing the hook:
  robo shell                 enter the runtime in-place and show <env> in the prompt
  robo deactivate            restore the previous prompt and environment"
    )]
    Hook(HookArgs),

    #[command(about = "Run project bootstrap scripts")]
    Bootstrap(PassthroughArgs),

    #[command(about = "Diagnose the current project runtime")]
    Doctor(check::CheckArgs),

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

    #[command(name = "__shell-env", hide = true)]
    InternalShellEnv,

    #[command(about = "Show help")]
    Help,
}

#[derive(Args)]
struct PassthroughArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

#[derive(Args)]
struct UpArgs {
    #[arg(default_value = ".", help = "Project directory to prepare")]
    target: PathBuf,

    #[arg(long, help = "Initialize missing runtime files without prompting")]
    yes: bool,

    #[arg(long, help = "Run uv sync after the native runtime is prepared")]
    sync: bool,

    #[arg(long, help = "Open an interactive runtime shell after setup succeeds")]
    shell: bool,
}

#[derive(Args)]
struct CompletionArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

#[derive(Args)]
struct HookArgs {
    #[arg(
        value_name = "SHELL",
        help = "Shell to print a hook for: bash, zsh, fish"
    )]
    shell: Option<OsString>,
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
    let config = Config { color, debug };
    console::set_colors_enabled(config.color);
    console::set_colors_enabled_stderr(config.color);

    match cli.command {
        Some(CliCommand::Init(args)) => init::run(args, config),
        Some(CliCommand::Up(args)) => {
            run_project_up(args.target, args.yes, args.sync, args.shell, config)
        }
        Some(CliCommand::Shell(args)) => run_project_shell(args.args, config),
        Some(CliCommand::Status) => run_project_status(config),
        Some(CliCommand::Deactivate) => run_project_deactivate(config),
        Some(CliCommand::Hook(args)) => run_project_hook(args.shell.into_iter().collect(), config),
        Some(CliCommand::Bootstrap(args)) => run_project_app(None, args.args, config),
        Some(CliCommand::Doctor(args)) => check::run(args, config),
        Some(CliCommand::Contract(args)) => contract::run(args, config),
        Some(CliCommand::DryRun(args)) => run_project_app(Some("--dry-run"), args.args, config),
        Some(CliCommand::Run(args)) => run_project_command(args.args, config),
        Some(CliCommand::Completion(args)) => print_completions(args.args, config),
        Some(CliCommand::CudaCheck) => cuda::check(config),
        Some(CliCommand::InternalExec(args)) => run_internal_exec(args.args, config),
        Some(CliCommand::InternalShellEnv) => run_internal_shell_env(config),
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
    println!("\n");
    help_section(config, "common workflows");
    help_row(
        config,
        "robo up",
        "prepare this project's native runtime",
    );
    help_row(
        config,
        "robo run <cmd>",
        "run a command inside the prepared runtime",
    );
    help_row(config, "robo doctor", "diagnose runtime and host prerequisites");
    help_row(config, "robo shell", "open an interactive runtime shell");

    println!();
    help_section(config, "prompt prefix");
    help_row(
        config,
        "eval \"$(robo hook)\"",
        "enable Conda-like in-place runtime shells",
    );
    help_row(
        config,
        "robo shell",
        "updates the current prompt, for example <simple>",
    );
    help_row(
        config,
        "robo deactivate",
        "restores the previous prompt and environment",
    );

    println!();
    help_section(config, "notes");
    println!(
        "  {}",
        label(
            config,
            "Without the hook, `robo shell` still works by opening a child runtime shell.",
            LabelKind::Hint,
        )
    );
    println!(
        "  {}",
        label(
            config,
            "Use `robo status` to see whether the current shell is active.",
            LabelKind::Hint,
        )
    );
    ExitCode::SUCCESS
}

fn help_section(config: Config, heading: &str) {
    println!("{}", label(config, heading, LabelKind::Status));
}

fn help_row(config: Config, command: &str, description: &str) {
    let padding = " ".repeat(22usize.saturating_sub(command.len()));
    println!(
        "  {}{} {}",
        label(config, command, LabelKind::Command),
        padding,
        label(config, description, LabelKind::Hint)
    );
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
    let shell = requested_shell_name(shell, "robo completion")?;
    match shell.as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        unknown => Err(format!("unsupported completion shell: {unknown}")),
    }
}

fn completion_error(config: Config, message: &str) -> ExitCode {
    error(config, message);
    crate::hint(
        config,
        &format!("supported shells: {SUPPORTED_INTERACTIVE_SHELLS}"),
    );
    ExitCode::from(2)
}
