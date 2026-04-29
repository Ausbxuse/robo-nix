use std::process::{Command, ExitCode};

use crate::{error, ok, output_with_spinner, status, Config, UiProgress};

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

pub(crate) fn run_bootstrap_with_progress(
    config: Config,
    progress: &mut UiProgress,
) -> Result<(), ExitCode> {
    let mut command = nix_command(config);
    command.arg("run").arg(".#default");
    command.env("ROBO_NIX_QUIET", "1");

    if config.debug {
        let status = run_status(&mut command, config);
        if status == ExitCode::SUCCESS {
            return Ok(());
        }
        return Err(status);
    }

    let output = progress.output(&mut command, "preparing runtime");
    progress.suspend(|| match output {
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
    })
}

fn run_bootstrap_output(
    command: &mut Command,
    config: Config,
) -> Result<std::process::Output, std::io::Error> {
    output_with_spinner(config, command, "preparing runtime")
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- bootstrap {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}
