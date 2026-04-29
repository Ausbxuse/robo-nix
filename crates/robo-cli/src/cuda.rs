use std::env;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::{error, hint, ok, status, Config};

const HOST_CUDA_DRIVER_LIBS: &[&str] = &[
    "/run/opengl-driver/lib/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/nvidia/current/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/nvidia/libcuda.so.1",
    "/usr/lib/wsl/lib/libcuda.so.1",
];

pub(crate) fn check(config: Config) -> ExitCode {
    status(config, "checking CUDA host prerequisites");
    if env::consts::OS != "linux" {
        error(config, "CUDA validation is only supported on Linux hosts.");
        return ExitCode::from(1);
    }
    if Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        error(
            config,
            "missing nvidia-smi; this host does not appear to have NVIDIA drivers installed.",
        );
        return ExitCode::from(1);
    }

    let cuda_root = env::var("CUDA_HOME")
        .or_else(|_| env::var("CUDA_PATH"))
        .unwrap_or_else(|_| "/usr/local/cuda".to_string());
    if !Path::new(&cuda_root).is_dir() {
        error(config, &format!("CUDA root not found at {cuda_root}"));
        hint(config, "set CUDA_HOME or CUDA_PATH if CUDA is installed elsewhere.");
        return ExitCode::from(1);
    }
    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            ok(config, "nvidia-smi is reachable");
            if let Some(path) = host_cuda_driver_lib() {
                ok(config, &format!("CUDA driver library visible at {path}"));
            } else {
                error(config, "libcuda.so.1 was not found in common host driver locations");
                hint(
                    config,
                    "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver.",
                );
                return ExitCode::from(1);
            }
            ok(config, &format!("CUDA root exists at {cuda_root}"));
            ExitCode::SUCCESS
        }
        _ => {
            error(config, "nvidia-smi failed; GPU driver stack is not healthy.");
            ExitCode::from(1)
        }
    }
}

fn host_cuda_driver_lib() -> Option<&'static str> {
    HOST_CUDA_DRIVER_LIBS
        .iter()
        .copied()
        .find(|path| Path::new(path).is_file())
}
