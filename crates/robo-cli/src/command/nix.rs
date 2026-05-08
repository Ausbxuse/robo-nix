use std::env;
use std::process::{Command, ExitCode, Stdio};

use crate::{error, hint, label, Config, LabelKind};

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
    nix_command_with_quiet(config, !config.debug)
}

fn nix_command_with_quiet(config: Config, quiet: bool) -> Command {
    let mut command = Command::new("nix");
    command.env("ROBO_NIX_COLOR", if config.color { "1" } else { "0" });
    command.args([
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
        "--accept-flake-config",
    ]);
    command.arg("--no-warn-dirty");
    if quiet {
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

pub(crate) fn command_for_runtime_progress(config: Config) -> Command {
    let mut command = nix_command_with_quiet(config, false);
    command.arg("-vv");
    if !config.debug {
        command.env("ROBO_NIX_QUIET", "1");
    }
    command
}

pub(crate) fn add_runtime_source_override(command: &mut Command) {
    if let Ok(source_url) = env::var("ROBO_NIX_RUNTIME_SOURCE_URL") {
        command
            .arg("--override-input")
            .arg("robo-nix")
            .arg(source_url);
    }
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

pub(super) fn hint_native_cuda_link_failure(config: Config, output: &std::process::Output) {
    let text = combined_output(output);
    let Some(library) = missing_native_cuda_link_library(&text) else {
        return;
    };

    hint(
        config,
        &format!(
            "native CUDA extension link failed while resolving `-l{library}`; Nix owns the CUDA compiler, headers, and link surface"
        ),
    );
    hint(
        config,
        "uv owns Python packages and nvidia-* CUDA runtime wheels such as cuBLAS, cuDNN, and NCCL.",
    );
    hint(
        config,
        "run `robo check --deep` to validate the cuda-toolkit build surface before changing project dependencies.",
    );
}

fn missing_native_cuda_link_library(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once("cannot find -l").map(|(_, rest)| rest))
        .filter_map(|rest| {
            rest.split(|ch: char| {
                ch.is_whitespace() || matches!(ch, ':' | ',' | ';' | '\'' | '"' | '`')
            })
            .find(|part| !part.is_empty())
        })
        .find(|library| is_cuda_link_library(library))
        .map(ToOwned::to_owned)
}

fn is_cuda_link_library(library: &str) -> bool {
    library.starts_with("cuda")
        || matches!(
            library,
            "cublas"
                | "cublasLt"
                | "cudart"
                | "cudart_static"
                | "cudnn"
                | "cufft"
                | "cufftw"
                | "cufile"
                | "cupti"
                | "curand"
                | "cusolver"
                | "cusolverMg"
                | "cusparse"
                | "cusparseLt"
                | "nccl"
                | "nvblas"
                | "nvJitLink"
                | "nvrtc"
                | "nvrtc-builtins"
                | "nvToolsExt"
                | "nvtx"
        )
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

#[cfg(test)]
mod tests {
    use super::missing_native_cuda_link_library;

    #[test]
    fn detects_cuda_link_library_failures() {
        let text = "/nix/store/bin/ld: cannot find -lcudart: No such file or directory";
        assert_eq!(
            missing_native_cuda_link_library(text),
            Some("cudart".to_string())
        );
    }

    #[test]
    fn ignores_non_cuda_link_library_failures() {
        let text = "/nix/store/bin/ld: cannot find -lssl: No such file or directory";
        assert_eq!(missing_native_cuda_link_library(text), None);
    }
}
