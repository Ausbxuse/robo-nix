use std::env;
use std::path::Path;

use crate::runtime::ProjectRuntime;
use crate::Config;

use super::output::{check_error, check_hint, check_ok, check_warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CudaCheckPlan {
    pub(super) expected_wheel_version: Option<String>,
    pub(super) host_required: bool,
    pub(super) toolkit_required: bool,
}

impl CudaCheckPlan {
    pub(super) fn needed(&self) -> bool {
        self.host_required || self.toolkit_required
    }
}

pub(super) fn cuda_check_plan(runtime: &ProjectRuntime) -> CudaCheckPlan {
    let expected_wheel_version = runtime
        .cuda_wheel_version
        .clone()
        .or_else(crate::runtime::infer_cuda_wheel_version_from_uv_lock);
    cuda_check_plan_from_expected(runtime, expected_wheel_version)
}

fn cuda_check_plan_from_expected(
    runtime: &ProjectRuntime,
    expected_wheel_version: Option<String>,
) -> CudaCheckPlan {
    let toolkit_required = runtime_has_component(runtime, "cuda-toolkit");
    let host_required =
        expected_wheel_version.is_some() || runtime_has_component(runtime, "isaac-sim");
    CudaCheckPlan {
        expected_wheel_version,
        host_required,
        toolkit_required,
    }
}

fn runtime_has_component(runtime: &ProjectRuntime, component: &str) -> bool {
    runtime.components.iter().any(|item| item == component)
}

pub(super) fn check_cuda_host(
    config: Config,
    runtime: &ProjectRuntime,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
) {
    let plan = cuda_check_plan(runtime);
    if !plan.needed() {
        return;
    }

    if plan.host_required {
        check_cuda_host_requirement(config, &plan, issues, warnings);
    }
    if plan.toolkit_required {
        check_cuda_toolkit_requirement(config, &plan, deep, issues, warnings);
    }
}

fn check_cuda_host_requirement(
    config: Config,
    plan: &CudaCheckPlan,
    issues: &mut usize,
    warnings: &mut usize,
) {
    if env::consts::OS != "linux" {
        check_error(config, issues, "CUDA environments require a Linux host");
        check_hint(
            config,
            "use a Linux NVIDIA machine for gpu-learning or isaac-learning environments",
        );
        return;
    }

    let host_cuda_version = crate::runtime::host_cuda_driver_version();
    if let Some(host_version) = host_cuda_version.as_deref() {
        check_ok(
            config,
            &format!("CUDA host driver supports {host_version}"),
        );
    } else {
        check_error(
            config,
            issues,
            "could not detect host NVIDIA driver CUDA support",
        );
        check_hint(
            config,
            "repair the host NVIDIA driver installation before using CUDA environments",
        );
    }

    if let Some(path) = crate::runtime::find_host_libcuda() {
        check_ok(config, &format!("CUDA driver library visible at {path}"));
        if env::var_os("ROBO_NIX_LIBCUDA_PATH").is_none()
            && plan.host_required
            && let Some(driver_dir) = Path::new(&path).parent()
        {
            check_hint(
                config,
                &format!(
                    "robo run/shell will add {} to the runtime automatically; set ROBO_NIX_LIBCUDA_PATH to override",
                    driver_dir.display()
                ),
            );
        }
    } else {
        check_warn(
            config,
            warnings,
            "libcuda.so.1 was not visible through ROBO_NIX_LIBCUDA_PATH, LD_LIBRARY_PATH, ldconfig, or known host driver locations",
        );
        check_hint(
            config,
            "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver",
        );
    }

    if let Some(expected_cuda_version) = plan.expected_wheel_version.as_deref() {
        if let Some(host_version) = host_cuda_version.as_deref() {
            if crate::runtime::cuda_version_less_than(&host_version, expected_cuda_version)
                == Some(true)
            {
                check_error(
                    config,
                    issues,
                    &format!(
                        "CUDA host driver mismatch: host supports {host_version}, uv.lock expects {expected_cuda_version}"
                    ),
                );
                check_hint(
                    config,
                    "upgrade the host NVIDIA driver or regenerate uv.lock with CUDA wheels supported by this host",
                );
            }
        }
    }
}

fn check_cuda_toolkit_requirement(
    config: Config,
    plan: &CudaCheckPlan,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
) {
    let Some(cuda_root) = crate::runtime::cuda_root_from_env() else {
        if deep {
            check_hint(
                config,
                "CUDA root is not visible in the current shell; deep checks will validate the runtime created by nix develop",
            );
            return;
        }
        check_warn(
            config,
            warnings,
            "CUDA root is not visible in the current shell",
        );
        check_hint(
            config,
            "robo shell sets CUDA_HOME/CUDA_PATH from the cuda-toolkit component",
        );
        if deep {
            check_hint(
                config,
                "deep checks will validate the runtime created by nix develop",
            );
        } else {
            check_hint(
                config,
                "open the runtime shell or run deep checks to validate the Nix CUDA toolkit",
            );
        }
        return;
    };
    check_ok(config, &format!("CUDA root exists at {cuda_root}"));

    let Some(expected_cuda_version) = plan.expected_wheel_version.as_deref() else {
        return;
    };

    let Some(actual_cuda_version) = crate::runtime::cuda_version_from_root() else {
        check_warn(
            config,
            warnings,
            "found CUDA root but could not detect its major.minor version",
        );
        check_hint(
            config,
            &format!(
                "run `robo shell -c \"$CUDA_HOME/bin/nvcc --version\"` to inspect this CUDA root"
            ),
        );
        return;
    };

    if actual_cuda_version == expected_cuda_version {
        check_ok(
            config,
            &format!("CUDA version alignment: {expected_cuda_version} at {cuda_root}"),
        );
    } else {
        check_error(
            config,
            issues,
            &format!(
                "CUDA mismatch: uv.lock expects {expected_cuda_version}, runtime reports {actual_cuda_version}"
            ),
        );
        check_hint(
            config,
            "point `ROBO_NIX_CUDA_ROOT` or `CUDA_HOME` to a toolkit matching expected CUDA ABI",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(components: &[&str], cuda_wheel_version: Option<&str>) -> ProjectRuntime {
        ProjectRuntime {
            schema_version: Some("1".to_string()),
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: cuda_wheel_version.map(ToOwned::to_owned),
            components: components.iter().map(|item| item.to_string()).collect(),
            suggestions: Vec::new(),
        }
    }

    #[test]
    fn cuda_wheels_require_host_but_not_toolkit() {
        let plan =
            cuda_check_plan_from_expected(&runtime(&[], Some("12.8")), Some("12.8".into()));

        assert!(plan.host_required);
        assert!(!plan.toolkit_required);
        assert_eq!(plan.expected_wheel_version.as_deref(), Some("12.8"));
    }

    #[test]
    fn cuda_toolkit_requires_build_surface_but_not_host_by_itself() {
        let plan = cuda_check_plan_from_expected(&runtime(&["cuda-toolkit"], None), None);

        assert!(!plan.host_required);
        assert!(plan.toolkit_required);
        assert_eq!(plan.expected_wheel_version, None);
    }

    #[test]
    fn isaac_sim_requires_cuda_host_even_before_lockfile_exists() {
        let plan = cuda_check_plan_from_expected(&runtime(&["isaac-sim"], None), None);

        assert!(plan.host_required);
        assert!(!plan.toolkit_required);
    }
}
