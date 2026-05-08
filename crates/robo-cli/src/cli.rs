use anstyle::AnsiColor;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::command::{
    run_internal_exec, run_internal_shell_env, run_internal_shell_refresh, run_project_app,
    run_project_build, run_project_command, run_project_shell, run_project_up,
};
use crate::shell::{SUPPORTED_INTERACTIVE_SHELLS, requested_shell_name};
use crate::{Config, LabelKind, check, contract, cuda, diagnose, error, init, label, search};

#[derive(Parser)]
#[command(
    name = "robo",
    about = "robo-nix project runtime helper",
    long_about = "Make pyproject.toml + uv work with the native robot-learning libraries they need, without requiring users to learn flakes first.",
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

    #[command(about = "Prebuild the project runtime cache")]
    Build(BuildArgs),

    #[command(about = "Prepare the project runtime", hide = true)]
    Up(UpArgs),

    #[command(about = "Open the project runtime shell")]
    Shell(PassthroughArgs),

    #[command(about = "Summarize project runtime health")]
    Status,

    #[command(about = "Diagnose project runtime and host prerequisites")]
    Check(check::CheckArgs),

    #[command(about = "Classify an existing runtime error log")]
    Diagnose(diagnose::DiagnoseArgs),

    #[command(about = "Search for the Nix package that provides a missing shared library")]
    Search(search::SearchArgs),

    #[command(about = "Run project bootstrap scripts")]
    Bootstrap(PassthroughArgs),

    #[command(about = "Print the detailed legacy runtime report")]
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

    #[command(name = "__shell-refresh", hide = true)]
    InternalShellRefresh(PassthroughArgs),

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

    #[arg(long, help = "Replace generated runtime files during setup")]
    force: bool,

    #[arg(long, help = "Open an interactive runtime shell after setup succeeds")]
    shell: bool,
}

#[derive(Args)]
struct BuildArgs {
    #[arg(default_value = ".", help = "Project directory to prebuild")]
    target: PathBuf,
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
    let config = Config { color, debug };
    console::set_colors_enabled(config.color);
    console::set_colors_enabled_stderr(config.color);

    match cli.command {
        Some(CliCommand::Init(args)) => init::run(args, config),
        Some(CliCommand::Build(args)) => run_project_build(args.target, config),
        Some(CliCommand::Up(args)) => {
            run_project_up(args.target, args.yes, args.force, args.shell, config)
        }
        Some(CliCommand::Shell(args)) => run_project_shell(args.args, config),
        Some(CliCommand::Status) => check::run_status(config),
        Some(CliCommand::Check(args)) => check::run_check(args, config),
        Some(CliCommand::Diagnose(args)) => diagnose::run(args, config),
        Some(CliCommand::Search(args)) => search::run(args, config),
        Some(CliCommand::Bootstrap(args)) => run_project_app(None, args.args, config),
        Some(CliCommand::Doctor(args)) => check::run(args, config),
        Some(CliCommand::Contract(args)) => contract::run(args, config),
        Some(CliCommand::DryRun(args)) => run_project_app(Some("--dry-run"), args.args, config),
        Some(CliCommand::Run(args)) => run_project_command(args.args, config),
        Some(CliCommand::Completion(args)) => print_completions(args.args, config),
        Some(CliCommand::CudaCheck) => cuda::check(config),
        Some(CliCommand::InternalExec(args)) => run_internal_exec(args.args, config),
        Some(CliCommand::InternalShellEnv) => run_internal_shell_env(config),
        Some(CliCommand::InternalShellRefresh(args)) => {
            run_internal_shell_refresh(args.args, config)
        }
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
        "robo init",
        "create or update runtime files",
    );
    help_row(
        config,
        "robo build",
        "prebuild and cache this project's native runtime",
    );
    help_row(
        config,
        "robo run <cmd>",
        "run a command inside the prepared runtime",
    );
    help_row(config, "robo check", "summarize runtime diagnostics");
    help_row(config, "robo check --deep", "run slower runtime probes");
    help_row(config, "robo diagnose -", "classify piped error logs");
    help_row(config, "robo search libassimp.so", "find a Nix runtime library package");
    help_row(config, "robo status", "summarize runtime health");
    help_row(config, "robo shell", "open an interactive runtime shell");

    println!();
    help_section(config, "runtime shell");
    help_row(
        config,
        "robo shell",
        "open a runtime shell with prompt prefix, for example [robo]",
    );
    help_row(config, "exit", "leave the active runtime shell");
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
