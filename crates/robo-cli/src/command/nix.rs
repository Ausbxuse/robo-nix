use std::process::{Command, ExitCode, Stdio};

use crate::{error, label, Config, LabelKind};

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

pub(crate) fn nix_command(config: Config) -> Command {
    let mut command = Command::new("nix");
    command.env("ROBO_NIX_COLOR", if config.color { "1" } else { "0" });
    command.args([
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
    ]);
    command.arg("--no-warn-dirty");
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

pub(super) fn run_status(command: &mut Command, config: Config) -> ExitCode {
    debug(config, command);
    match command.status() {
        Ok(status) => exit_code(status.code()),
        Err(err) => {
            error(config, &format!("failed to start command: {err}"));
            ExitCode::from(1)
        }
    }
}

pub(super) fn exit_code(code: Option<i32>) -> ExitCode {
    match code {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        _ => ExitCode::from(1),
    }
}

pub(super) fn check_command(name: &str) -> Result<(), String> {
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

fn debug(config: Config, command: &Command) {
    if config.debug {
        eprintln!(
            "{} {:?}",
            label(config, "debug:", LabelKind::Debug),
            command
        );
    }
}
