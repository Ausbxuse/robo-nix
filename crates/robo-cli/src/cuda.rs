use std::process::ExitCode;

use crate::{Config, error, hint, ok, status};

pub(crate) fn check(config: Config) -> ExitCode {
    status(config, "checking CUDA host prerequisites");
    if std::env::consts::OS != "linux" {
        error(config, "CUDA validation is only supported on Linux hosts.");
        return ExitCode::from(1);
    }
    let Some(host_version) = crate::runtime::host_cuda_driver_version() else {
        error(config, "could not detect host NVIDIA driver CUDA support.");
        hint(config, "repair the host NVIDIA driver installation.");
        return ExitCode::from(1);
    };

    ok(config, &format!("CUDA host driver supports {host_version}"));
    if let Some(path) = crate::runtime::find_host_libcuda() {
        ok(config, &format!("CUDA driver library visible at {path}"));
    } else {
        error(
            config,
            "libcuda.so.1 was not visible through ROBO_NIX_LIBCUDA_PATH, LD_LIBRARY_PATH, ldconfig, or known host driver locations",
        );
        hint(
            config,
            "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver.",
        );
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
