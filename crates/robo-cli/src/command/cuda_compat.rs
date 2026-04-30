use std::path::Path;
use std::process::ExitCode;

use crate::{error, hint, ok, warn, Config};

use super::nix::command_for_runtime;

pub(super) fn ensure_runtime_cuda_compat(config: Config, strict: bool) -> Result<(), ExitCode> {
    let runtime = crate::runtime::read_project_runtime();
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        return Ok(());
    }

    let expected_version = runtime
        .cuda_wheel_version
        .clone()
        .or_else(crate::runtime::infer_cuda_wheel_version_from_uv_lock);
    let Some(expected_version) = expected_version else {
        if Path::new("uv.lock").exists() {
            hint(
                config,
                "robo.nix does not contain `cudaWheelVersion`; run `robo init . --force` to regenerate metadata.",
            );
        } else {
            hint(
                config,
                "create/update uv.lock so robo can infer CUDA runtime expectations.",
            );
        }
        return Ok(());
    };

    match crate::runtime::host_cuda_driver_version() {
        Some(host_version) => {
            if crate::runtime::cuda_version_less_than(&host_version, &expected_version)
                == Some(true)
            {
                let message = format!(
                    "host NVIDIA driver supports CUDA {host_version}, but uv.lock expects CUDA {expected_version}"
                );
                if strict {
                    error(config, &message);
                } else {
                    warn(config, &message);
                }
                hint(
                    config,
                    "upgrade the host NVIDIA driver or regenerate uv.lock with CUDA wheels supported by this host.",
                );
                return if strict {
                    Err(ExitCode::from(1))
                } else {
                    Ok(())
                };
            }
        }
        None => {
            let message = "could not detect host NVIDIA driver CUDA support with nvidia-smi";
            if strict {
                error(config, message);
            } else {
                warn(config, message);
            }
            hint(
                config,
                "repair the host NVIDIA driver before running CUDA/Isaac workloads.",
            );
            return if strict {
                Err(ExitCode::from(1))
            } else {
                Ok(())
            };
        }
    }

    let Some((root, actual_version)) = probe_runtime_cuda(config) else {
        let message = format!(
            "runtime CUDA toolkit is not available inside nix develop; expected CUDA {expected_version}"
        );
        if strict {
            error(config, &message);
        } else {
            warn(config, &message);
        }
        hint(
            config,
            "run `robo check --deep` and check the cuda-toolkit component in robo.nix.",
        );
        return if strict {
            Err(ExitCode::from(1))
        } else {
            Ok(())
        };
    };

    if actual_version != expected_version {
        let message = format!(
            "runtime CUDA mismatch: uv.lock expects CUDA {expected_version}, nix develop provides {actual_version}"
        );
        if strict {
            error(config, &message);
        } else {
            warn(config, &message);
        }
        hint(
            config,
            "align cudaWheelVersion, uv.lock CUDA wheels, or the cuda-toolkit component before building CUDA extensions.",
        );
        return if strict {
            Err(ExitCode::from(1))
        } else {
            Ok(())
        };
    }

    ok(
        config,
        &format!("runtime CUDA compatibility check passed ({expected_version} at {root})"),
    );
    Ok(())
}

fn probe_runtime_cuda(config: Config) -> Option<(String, String)> {
    let script = r#"root="${ROBO_NIX_CUDA_ROOT:-${CUDA_HOME:-${CUDA_PATH:-}}}"
printf 'root=%s\n' "$root"
if [ -n "$root" ] && [ -x "$root/bin/nvcc" ]; then
  "$root/bin/nvcc" --version
elif command -v nvcc >/dev/null 2>&1; then
  nvcc --version
fi"#;
    let output = command_for_runtime(config)
        .arg("develop")
        .arg("-c")
        .arg("bash")
        .arg("-lc")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let root = stdout
        .lines()
        .find_map(|line| line.strip_prefix("root="))
        .filter(|root| !root.is_empty())?
        .to_string();
    let version = crate::runtime::cuda_release_version_from_text(&stdout)
        .or_else(|| crate::runtime::cuda_release_version_from_text(&stderr))?;
    Some((root, version))
}
