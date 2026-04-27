use anstyle::AnsiColor;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

mod contract;
mod doctor;
mod init;
mod runtime;
mod vendor;

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

    #[command(about = "Run project bootstrap scripts")]
    Bootstrap(PassthroughArgs),

    #[command(about = "Diagnose the current project runtime")]
    Doctor(doctor::DoctorArgs),

    #[command(about = "Print the resolved runtime contract")]
    Contract(contract::ContractArgs),

    #[command(about = "Inspect project-owned vendor source trees")]
    Vendor(vendor::VendorArgs),

    #[command(
        name = "dry-run",
        about = "Validate bootstrap without entering a shell"
    )]
    DryRun(PassthroughArgs),

    #[command(about = "Run uv sync inside the Nix runtime")]
    Sync(PassthroughArgs),

    #[command(about = "Enter nix develop")]
    Develop(PassthroughArgs),

    #[command(about = "Run a Python command with uv inside nix develop")]
    Run(PassthroughArgs),

    #[command(hide = true)]
    Completion(CompletionArgs),

    #[command(name = "cuda-doctor", hide = true)]
    CudaDoctor,

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

fn main() -> ExitCode {
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
        Some(CliCommand::Bootstrap(args)) => run_project_app(None, args.args, config),
        Some(CliCommand::Doctor(args)) => doctor::run(args, config),
        Some(CliCommand::Contract(args)) => contract::run(args, config),
        Some(CliCommand::Vendor(args)) => vendor::run(args, config),
        Some(CliCommand::DryRun(args)) => run_project_app(Some("--dry-run"), args.args, config),
        Some(CliCommand::Sync(args)) => run_uv_sync(args.args, config),
        Some(CliCommand::Develop(args)) => run_nix_develop(args.args, config),
        Some(CliCommand::Run(args)) => run_project_command(args.args, config),
        Some(CliCommand::Completion(args)) => print_completions(args.args),
        Some(CliCommand::CudaDoctor) => init::cuda_doctor(config),
        Some(CliCommand::Help) | None => {
            let mut command = Cli::command();
            if let Err(err) = command.print_help() {
                error(config, &format!("failed to print help: {err}"));
                return ExitCode::from(1);
            }
            println!();
            ExitCode::SUCCESS
        }
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

#[derive(Clone, Copy)]
pub(crate) struct Config {
    color: bool,
    debug: bool,
}

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

pub(crate) fn combined_output(output: &std::process::Output) -> String {
    let mut text = String::new();
    text.push_str(String::from_utf8_lossy(&output.stdout).trim());
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(String::from_utf8_lossy(&output.stderr).trim());
    }
    if text.is_empty() {
        format!("command exited with {}", output.status)
    } else {
        text
    }
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
        hint(config, "run `robo init . --force` only if you want robo to replace this flake.");
        return Err(ExitCode::from(1));
    }
    if !flake.contains("github:ausbxuse/robo-nix") {
        return Ok(());
    }

    let source_url = match env::var("ROBO_NIX_DEFAULT_SOURCE_URL") {
        Ok(source_url) => source_url,
        Err(_) => {
            error(config, "flake.nix points at github:ausbxuse/robo-nix, but this robo install has no packaged source URL.");
            hint(config, "run `robo init . --robo-nix-url path:/path/to/robo-nix` to repair this project.");
            return Err(ExitCode::from(1));
        }
    };
    let repaired = flake.replace("github:ausbxuse/robo-nix", &source_url);
    if let Err(err) = fs::write(flake_path, repaired) {
        error(config, &format!("failed to repair flake.nix: {err}"));
        return Err(ExitCode::from(1));
    }
    status(config, &format!("repaired flake.nix to use {source_url}"));
    Ok(())
}

fn run_project_app(mode: Option<&str>, args: Vec<OsString>, config: Config) -> ExitCode {
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

pub(crate) fn run_bootstrap(config: Config) -> Result<(), ExitCode> {
    let mut command = nix_command(config);
    command.arg("run").arg(".#default");
    command.env("ROBO_NIX_QUIET", "1");

    if config.debug {
        status(config, "preparing runtime");
        let status = run_status(&mut command, config);
        if status == ExitCode::SUCCESS {
            return Ok(());
        }
        return Err(status);
    }

    match run_bootstrap_output(&mut command, config) {
        Ok(output) if output.status.success() => {
            ok(config, "runtime ready");
            Ok(())
        }
        Ok(output) => {
            error(config, "runtime bootstrap failed");
            print_captured("stdout", &output.stdout);
            print_captured("stderr", &output.stderr);
            Err(exit_code(output.status.code()))
        }
        Err(err) => {
            error(config, &format!("failed to start bootstrap: {err}"));
            Err(ExitCode::from(1))
        }
    }
}

fn run_bootstrap_output(
    command: &mut Command,
    config: Config,
) -> Result<std::process::Output, std::io::Error> {
    if !std::io::stderr().is_terminal() {
        status(config, "preparing runtime");
        return command.output();
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let spinner = spinner(config, "preparing runtime");
    let started_at = Instant::now();
    let output = command.output();
    keep_spinner_visible(started_at);
    spinner.finish_and_clear();
    output
}

pub(crate) fn nix_command(config: Config) -> Command {
    let mut command = Command::new("nix");
    command.env("ROBO_NIX_COLOR", if config.color { "1" } else { "0" });
    command.args([
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
    ]);
    if !config.debug {
        command.arg("--quiet");
    }
    command
}

pub(crate) fn command_for_runtime(config: Config) -> Command {
    let mut command = nix_command(config);
    if !config.debug {
        command.env("ROBO_NIX_QUIET", "1");
    }
    command
}

fn exit_code(code: Option<i32>) -> ExitCode {
    match code {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        _ => ExitCode::from(1),
    }
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- bootstrap {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}

fn run_uv_sync(args: Vec<OsString>, config: Config) -> ExitCode {
    if !Path::new("pyproject.toml").exists() {
        error(config, "sync needs pyproject.toml.");
        hint(config, "run `robo init .` or create pyproject.toml for uv.");
        return ExitCode::from(1);
    }
    if let Err(code) = ensure_python_version_files(config) {
        return code;
    }

    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }
    if let Err(code) = run_bootstrap(config) {
        return code;
    }

    status(config, "syncing Python environment");
    // NOTE: uv sync may build native extensions. Run it inside the Nix runtime
    // so compiler and library paths match what the project will use.
    run_status(
        command_for_runtime(config)
            .arg("develop")
            .arg("-c")
            .arg("uv")
            .arg("sync")
            .args(args),
        config,
    )
}

fn run_nix_develop(args: Vec<OsString>, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }
    if let Err(code) = run_bootstrap(config) {
        return code;
    }

    run_status(
        command_for_runtime(config).arg("develop").args(args),
        config,
    )
}

fn run_project_command(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "run needs a command.");
        hint(config, "example: robo run pytest tests");
        return ExitCode::from(2);
    }

    if !Path::new("pyproject.toml").exists() {
        error(config, "run needs pyproject.toml.");
        hint(config, "run `robo init .` or create pyproject.toml for uv.");
        return ExitCode::from(1);
    }
    if let Err(code) = ensure_python_version_files(config) {
        return code;
    }

    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }
    if let Err(code) = run_bootstrap(config) {
        return code;
    }

    let args = if args.get(0).is_some_and(|arg| arg == "uv")
        && args.get(1).is_some_and(|arg| arg == "run")
    {
        args.into_iter().skip(2).collect()
    } else {
        args
    };

    run_status(
        command_for_runtime(config)
            .arg("develop")
            .arg("-c")
            .arg("uv")
            .arg("run")
            .args(args),
        config,
    )
}

fn ensure_python_version_files(config: Config) -> Result<(), ExitCode> {
    let Ok(pyproject) = fs::read_to_string("pyproject.toml") else {
        return Ok(());
    };
    let Some(required) = exact_python_requirement(&pyproject) else {
        return Ok(());
    };
    let Ok(project_python) = fs::read_to_string(".python-version") else {
        error(
            config,
            &format!("pyproject.toml requires Python {required}, but .python-version is missing."),
        );
        hint(config, &format!("write `{required}` to .python-version, then rerun this command."));
        return Err(ExitCode::from(1));
    };
    let project_python = project_python.trim();

    if project_python == required {
        if let Ok(robo_nix) = fs::read_to_string("robo.nix") {
            if let Some(robo_python) = robo_python_version(&robo_nix) {
                if robo_python != required {
                    error(
                        config,
                        &format!("robo.nix declares Python {robo_python}, but pyproject.toml requires Python {required}."),
                    );
                    hint(config, &format!("set `pythonVersion = \"{required}\";` in robo.nix."));
                    return Err(ExitCode::from(1));
                }
            }
        }
        return Ok(());
    }

    error(
        config,
        &format!(".python-version is {project_python}, but pyproject.toml requires Python {required}."),
    );
    hint(config, &format!("write `{required}` to .python-version, then rerun this command."));
    hint(config, "if robo.nix has a different pythonVersion, update that to match too.");
    Err(ExitCode::from(1))
}

fn robo_python_version(text: &str) -> Option<&str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("pythonVersion") else {
            continue;
        };
        return quoted_value(value);
    }
    None
}

pub(crate) fn exact_python_requirement(text: &str) -> Option<&str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("requires-python") else {
            continue;
        };
        let Some(raw) = quoted_value(value) else {
            continue;
        };
        let raw = raw.trim();
        let Some(rest) = raw.strip_prefix("===").or_else(|| raw.strip_prefix("==")) else {
            continue;
        };
        return rest
            .trim_start()
            .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
            .find(|part| {
                matches!(part.matches('.').count(), 1 | 2)
                    && part.split('.').all(|item| !item.is_empty() && item.chars().all(|ch| ch.is_ascii_digit()))
            });
    }
    None
}

pub(crate) fn quoted_value(text: &str) -> Option<&str> {
    let (_, value) = text.split_once('=')?;
    let value = value.trim();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &value[quote.len_utf8()..];
    let end = body.find(quote)?;
    Some(&body[..end])
}

fn spinner(config: Config, message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_draw_target(ProgressDrawTarget::stderr());
    spinner.set_style(
        ProgressStyle::with_template("{prefix} {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_prefix(label(config, "robo:", LabelKind::Status));
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

fn keep_spinner_visible(started_at: Instant) {
    let minimum = Duration::from_millis(450);
    let elapsed = started_at.elapsed();
    if elapsed < minimum {
        std::thread::sleep(minimum - elapsed);
    }
}

pub(crate) enum LabelKind {
    Status,
    Ok,
    Warn,
    Error,
    Hint,
    Why,
    Debug,
}

pub(crate) fn label(config: Config, text: &str, kind: LabelKind) -> String {
    if !config.color {
        return text.to_string();
    }

    match kind {
        LabelKind::Status => style(text).cyan().bold().to_string(),
        LabelKind::Ok => style(text).green().bold().to_string(),
        LabelKind::Warn => style(text).yellow().bold().to_string(),
        LabelKind::Error => style(text).red().bold().to_string(),
        LabelKind::Hint => style(text).dim().to_string(),
        LabelKind::Why => style(text).magenta().bold().to_string(),
        LabelKind::Debug => style(text).magenta().bold().to_string(),
    }
}

pub(crate) fn status(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "robo:", LabelKind::Status));
}

pub(crate) fn ok(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "ok:", LabelKind::Ok));
}

pub(crate) fn error(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "error:", LabelKind::Error));
}

pub(crate) fn hint(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "hint:", LabelKind::Hint));
}

fn debug(config: Config, command: &Command) {
    if config.debug {
        eprintln!(
            "{} {:?}",
            label(config, "debug:", LabelKind::Debug),
            command
        );
    }
}

fn run_status(command: &mut Command, config: Config) -> ExitCode {
    debug(config, command);
    match command.status() {
        Ok(status) => exit_code(status.code()),
        Err(err) => {
            error(config, &format!("failed to start command: {err}"));
            ExitCode::from(1)
        }
    }
}

fn check_command(name: &str) -> Result<(), String> {
    match Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(format!("`{name}` exists but did not run successfully.")),
        Err(_) => Err(format!("`{name}` was not found on PATH.")),
    }
}

fn print_completions(args: Vec<OsString>) -> ExitCode {
    let shell = match completion_shell(args.first()) {
        Ok(shell) => shell,
        Err(message) => return completion_error(&message),
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
        unknown => Err(format!("robo: unsupported completion shell: {unknown}")),
    }
}

fn completion_error(message: &str) -> ExitCode {
    eprintln!("{message}");
    eprintln!("supported shells: bash, zsh, fish");
    ExitCode::from(2)
}
