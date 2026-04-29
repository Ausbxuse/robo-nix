use std::io::{Read, Write};
use std::process::{Command, ExitCode, Stdio};
use std::thread;

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

pub(super) fn run_status_after_marker(
    command: &mut Command,
    config: Config,
    marker: &str,
) -> ExitCode {
    if config.debug {
        return run_status(command, config);
    }

    debug(config, command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            error(config, &format!("failed to start command: {err}"));
            return ExitCode::from(1);
        }
    };

    let marker = marker.as_bytes().to_vec();
    let stdout = child.stdout.take().map(|stream| {
        let marker = marker.clone();
        thread::spawn(move || stream_after_marker(stream, std::io::stdout(), &marker))
    });
    let stderr = child.stderr.take().map(|stream| {
        let marker = marker.clone();
        thread::spawn(move || stream_after_marker(stream, std::io::stderr(), &marker))
    });

    let status = match child.wait() {
        Ok(status) => status,
        Err(err) => {
            error(config, &format!("failed to wait for command: {err}"));
            return ExitCode::from(1);
        }
    };

    let stdout = stdout
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    if !status.success() && !stdout.marker_seen && !stderr.marker_seen {
        error(config, "runtime command failed before user command started");
        print_captured("stdout", &stdout.captured);
        print_captured("stderr", &stderr.captured);
    }

    exit_code(status.code())
}

#[derive(Default)]
struct MarkedOutput {
    captured: Vec<u8>,
    marker_seen: bool,
}

fn stream_after_marker(
    mut reader: impl Read,
    mut writer: impl Write,
    marker: &[u8],
) -> MarkedOutput {
    let mut output = MarkedOutput::default();
    let mut pending = Vec::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        let chunk = &buffer[..read];
        output.captured.extend_from_slice(chunk);

        if output.marker_seen {
            let _ = writer.write_all(chunk);
            let _ = writer.flush();
            continue;
        }

        pending.extend_from_slice(chunk);
        if let Some(index) = find_bytes(&pending, marker) {
            output.marker_seen = true;
            let after = &pending[index + marker.len()..];
            let _ = writer.write_all(after);
            let _ = writer.flush();
            pending.clear();
        } else if pending.len() > marker.len() {
            let keep = marker.len().saturating_sub(1);
            let drain_to = pending.len() - keep;
            pending.drain(..drain_to);
        }
    }

    output
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- runtime {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
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
