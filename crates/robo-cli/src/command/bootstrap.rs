use std::process::{Command, ExitCode};

use crate::{error, hint, ok, output_with_spinner, status, Config, UiProgress};

use super::nix::{
    add_runtime_source_override, exit_code, hint_native_cuda_link_failure, nix_command, run_status,
};

const BOOTSTRAP_MESSAGE: &str = "bootstrap: running project bootstrap";

pub(crate) fn run_bootstrap(config: Config) -> Result<(), ExitCode> {
    let mut command = nix_command(config);
    command.arg("run");
    add_runtime_source_override(&mut command);
    command.arg(".#default");
    command.env("ROBO_NIX_QUIET", "1");

    if config.debug {
        status(config, BOOTSTRAP_MESSAGE);
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
            hint_project_bootstrap_failure(config, &output);
            hint_native_cuda_link_failure(config, &output);
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
    command.arg("run");
    add_runtime_source_override(&mut command);
    command.arg(".#default");
    command.env("ROBO_NIX_QUIET", "1");

    if config.debug {
        let status = run_status(&mut command, config);
        if status == ExitCode::SUCCESS {
            return Ok(());
        }
        return Err(status);
    }

    let output = progress.output(&mut command, BOOTSTRAP_MESSAGE);
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => {
            ok(config, "runtime ready");
            Ok(())
        }
        Ok(output) => {
            error(config, "runtime bootstrap failed");
            print_captured("stdout", &output.stdout);
            print_captured("stderr", &output.stderr);
            hint_project_bootstrap_failure(config, &output);
            hint_native_cuda_link_failure(config, &output);
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
    output_with_spinner(config, command, BOOTSTRAP_MESSAGE)
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- bootstrap {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}

fn hint_project_bootstrap_failure(config: Config, output: &std::process::Output) {
    let text = super::nix::combined_output(output);
    hint(
        config,
        "bootstrap is project-owned code from the bootstrap block or source scripts in robo.nix.",
    );
    if let Some(name) = missing_env_var(&text) {
        hint(
            config,
            &format!("set `{name}` in your shell or map it from a robo-nix runtime variable in robo.nix."),
        );
    }
    hint(
        config,
        "run `robo check --why` to see which bootstrap scripts are part of this runtime.",
    );
}

fn missing_env_var(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let name = line.trim().strip_suffix(" is not set")?;
        is_env_var_name(name).then_some(name)
    })
}

fn is_env_var_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_env_var_from_bootstrap_output() {
        assert_eq!(
            missing_env_var("PROJECT_SDK_ROOT is not set"),
            Some("PROJECT_SDK_ROOT")
        );
    }

    #[test]
    fn ignores_non_env_var_bootstrap_output() {
        assert_eq!(missing_env_var("some lowercase setting is not set"), None);
    }
}
