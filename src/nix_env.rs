use std::process::Command;

use crate::error::AppError;
use crate::ui::{output_with_tree, Config};

const ENV_START_MARKER: &[u8] = b"robo-nix-env-start";
const ENV_CAPTURE_SCRIPT: &str = "printf '\\000robo-nix-env-start\\000'; env -0";

pub(crate) fn dev_environment(
    config: Config,
    phase: &str,
) -> Result<Vec<(String, String)>, AppError> {
    let mut command = Command::new("nix");
    command
        .arg("develop")
        .arg("--accept-flake-config")
        .arg("--command")
        .arg("sh")
        .arg("-c")
        .arg(ENV_CAPTURE_SCRIPT);
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
        return parse_env_zero(&output.stdout).map_err(AppError::project);
    }

    crate::write_command_output(&output)?;
    Err(AppError::project(format!(
        "nix develop exited with {}",
        output.status
    ))
    .with_hint("review the Nix output above and attach .robo-nix/last-error.log to an issue if this looks like a robo-nix bug."))
}

pub(crate) fn parse_env_zero(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut envs = Vec::new();
    let entries = bytes.split(|byte| *byte == 0);
    let entries = match bytes
        .split(|byte| *byte == 0)
        .position(|entry| entry == ENV_START_MARKER)
    {
        Some(marker) => entries.skip(marker + 1).collect::<Vec<_>>(),
        None => entries.collect::<Vec<_>>(),
    };

    for entry in entries {
        if entry.is_empty() {
            continue;
        }
        let Some(eq) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let name = String::from_utf8(entry[..eq].to_vec())
            .map_err(|_| "dev shell environment contains an invalid variable name")?;
        let value = String::from_utf8(entry[eq + 1..].to_vec())
            .map_err(|_| "dev shell environment contains an invalid variable value")?;
        envs.push((name, value));
    }
    Ok(envs)
}

pub(crate) fn apply_env(command: &mut Command, envs: &[(String, String)]) {
    command.envs(envs.iter().map(|(name, value)| (name, value)));
}

pub(crate) fn add_env_capture_args(command: &mut Command) {
    command.arg("sh").arg("-c").arg(ENV_CAPTURE_SCRIPT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nul_separated_shell_environment() {
        assert_eq!(
            parse_env_zero(b"PATH=/bin\0BAD\0QUOTE=a'b\0").unwrap(),
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("QUOTE".to_string(), "a'b".to_string()),
            ]
        );
    }

    #[test]
    fn ignores_shell_hook_stdout_before_marker() {
        assert_eq!(
            parse_env_zero(b"hello from shell hook\n\0robo-nix-env-start\0PATH=/bin\0").unwrap(),
            vec![("PATH".to_string(), "/bin".to_string())]
        );
    }
}
