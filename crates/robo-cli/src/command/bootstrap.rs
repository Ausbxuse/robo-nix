use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::IsTerminal;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use crate::{error, label, ok, status, Config, LabelKind};

use super::nix::{exit_code, nix_command, run_status};

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

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- bootstrap {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}
