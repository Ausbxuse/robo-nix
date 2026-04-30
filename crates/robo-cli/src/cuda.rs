use std::env;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::{error, hint, ok, status, Config};

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
            if let Some(path) = crate::runtime::find_host_libcuda() {
                ok(config, &format!("CUDA driver library visible at {path}"));
            } else {
                error(
                    config,
                    "libcuda.so.1 was not visible through ROBO_NIX_LIBCUDA_PATH, LD_LIBRARY_PATH, or ldconfig",
                );
                hint(
                    config,
                    "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver.",
                );
                return ExitCode::from(1);
            }
            if let Err(err) = crate::runtime::probe_libcuda_driver_entrypoint("cuDeviceGetUuid") {
                error(
                    config,
                    &format!("CUDA driver entry point unavailable: cuDeviceGetUuid ({err})"),
                );
                hint(
                    config,
                    "Warp and Isaac require a host NVIDIA driver whose libcuda.so.1 exports this driver API.",
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

pub(crate) fn driver_probe(config: Config) -> ExitCode {
    match crate::runtime::probe_libcuda_driver_entrypoint("cuDeviceGetUuid") {
        Ok(()) => {
            ok(config, "CUDA driver entry point available: cuDeviceGetUuid");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error(
                config,
                &format!("CUDA driver entry point unavailable: cuDeviceGetUuid ({err})"),
            );
            hint(
                config,
                "Warp and Isaac require a host NVIDIA driver whose libcuda.so.1 exports this driver API.",
            );
            ExitCode::from(1)
        }
    }
}
