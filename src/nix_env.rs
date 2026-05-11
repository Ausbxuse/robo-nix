use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::AppError;
use crate::inference::dependency_names_from_pyproject;
use crate::ui::{output_with_tree, Config};

const ENV_START_MARKER: &[u8] = b"robo-nix-env-start";
const ENV_CAPTURE_SCRIPT: &str = "printf '\\000robo-nix-env-start\\000'; env -0";
const LIBCUDA_NAMES: &[&str] = &["libcuda.so.1", "libcuda.so"];
const KNOWN_HOST_LIBCUDA_DIRS: &[&str] = &[
    "/run/opengl-driver/lib",
    "/usr/lib64/nvidia",
    "/usr/lib/x86_64-linux-gnu",
    "/usr/lib/wsl/lib",
];
const KNOWN_HOST_NVIDIA_LIBRARY_DIRS: &[&str] = &[
    "/run/opengl-driver/lib",
    "/usr/lib64/nvidia",
    "/usr/lib/x86_64-linux-gnu/nvidia/current",
    "/usr/lib/x86_64-linux-gnu",
    "/lib/x86_64-linux-gnu",
];
const KNOWN_HOST_NVIDIA_EGL_PLATFORM_DIRS: &[&str] = &[
    "/run/opengl-driver/share/egl/egl_external_platform.d",
    "/usr/share/egl/egl_external_platform.d",
];
const KNOWN_HOST_NVIDIA_GBM_BACKEND_DIRS: &[&str] = &[
    "/run/opengl-driver/lib/gbm",
    "/run/opengl-driver/lib",
    "/usr/lib/x86_64-linux-gnu/gbm",
    "/usr/lib/x86_64-linux-gnu/nvidia/current",
    "/usr/lib64/gbm",
    "/usr/lib64/nvidia",
    "/usr/lib/gbm",
];
const REQUIRED_NVIDIA_GRAPHICS_LIBRARIES: &[&str] = &["libEGL_nvidia.so.0", "libGLX_nvidia.so.0"];
const COMMON_NVIDIA_GRAPHICS_LIBRARIES: &[&str] = &[
    "libGLESv1_CM_nvidia.so.1",
    "libGLESv2_nvidia.so.2",
    "libnvidia-egl-gbm.so.1",
    "libnvidia-egl-wayland.so.1",
];
const NVIDIA_DRIVER_LIBRARY_PREFIXES: &[&str] = &[
    "libEGL_nvidia.so",
    "libGLX_nvidia.so",
    "libGLESv1_CM_nvidia.so",
    "libGLESv2_nvidia.so",
    "libnvidia-",
    "libnvcuvid.so",
    "libnvoptix.so",
];
const HOST_CUDA_DEPENDENCIES: &[&str] = &[
    "cuda-python",
    "cupy",
    "cupy-cuda11x",
    "cupy-cuda12x",
    "isaacsim",
    "nvidia-curobo",
];
const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 30;
const RUNTIME_ENV_CACHE_MAGIC: &str = "robo-nix-runtime-env-cache-v1";
const RUNTIME_ENV_CACHE_FILE: &str = "runtime-env-cache-v1.env0";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum EnvVarVisibility {
    Public,
    Internal,
    External,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct EnvVarSpec {
    pub(crate) name: &'static str,
    pub(crate) visibility: EnvVarVisibility,
    pub(crate) affects_runtime_key: bool,
    pub(crate) description: &'static str,
}

pub(crate) const ENV_VARS: &[EnvVarSpec] = &[
    EnvVarSpec {
        name: "ROBO_NIX_SHELL",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: false,
        description: "Explicit interactive shell override for `robo shell`.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_DEBUG",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: false,
        description: "Enable debug output and plain progress rendering.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_NO_SPINNER",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: false,
        description: "Disable spinner/progress tree rendering.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_LIBCUDA_PATH",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Explicit host libcuda.so.1 file or containing directory.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_DISABLE_HOST_CUDA_AUTO",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Disable automatic host libcuda.so.1 bridge probing.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_LOCK_TIMEOUT",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: false,
        description: "Seconds to wait for robo-owned project lock files.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_DEFAULT_SOURCE_URL",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: false,
        description: "Override the generated flake input URL for local development.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_ACTIVE",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Marks an active robo shell.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_ENV_NAME",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Current robo environment name.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_PROMPT_PREFIX",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Enables prompt integration in generated shell startup files.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_PARENT_ZDOTDIR",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Original zsh dotfile directory for prompt-preserving zsh launch.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_RUNTIME_INPUT_KEY",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Active shell runtime input fingerprint.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_RUNTIME_INPUT_FILES",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Active shell runtime input file fingerprints.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_COMPONENTS",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Selected runtime components reported by the Nix shell.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_MANAGED_ENV_VARS",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Variables managed by the active shell refresh delta.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_LIBCUDA_AUTO",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Detected host CUDA driver directory.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_LIBCUDA_BRIDGE",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Robo-owned symlink bridge for host libcuda without host glibc.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Host driver directory skipped because it also exposed glibc.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_CUDA_ROOT",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Override the Nix CUDA toolkit root used by the cuda-toolkit component.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_NVIDIA_VK_ICD",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Override the Vulkan ICD path selected by hostGraphics = \"nvidia\".",
    },
    EnvVarSpec {
        name: "ROBO_NIX_NVIDIA_EGL_VENDOR",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Override the EGL vendor JSON path selected by hostGraphics = \"nvidia\".",
    },
    EnvVarSpec {
        name: "ROBO_NIX_NVIDIA_DRIVER_LIB_DIR",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Override the host NVIDIA userspace driver library directory selected by hostGraphics = \"nvidia\".",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_GRAPHICS",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Selected host graphics provider policy reported by the Nix shell.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_GRAPHICS_EGL_PLATFORM_DIR",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Robo-owned EGL external platform config directory for host graphics.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_GRAPHICS_GBM_BACKEND_DIR",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Robo-owned GBM backend directory for host graphics.",
    },
    EnvVarSpec {
        name: "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "EGL external platform config search path inherited from the host session.",
    },
    EnvVarSpec {
        name: "GBM_BACKENDS_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "GBM backend search path inherited from the host session.",
    },
    EnvVarSpec {
        name: "WORKSPACE_ROOT",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Workspace root propagated into active shells.",
    },
    EnvVarSpec {
        name: "LD_LIBRARY_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "Host library path consulted for explicit CUDA driver visibility.",
    },
];

pub(crate) fn runtime_key_env_names() -> impl Iterator<Item = &'static str> {
    ENV_VARS
        .iter()
        .filter(|spec| spec.affects_runtime_key)
        .map(|spec| spec.name)
}

pub(crate) fn is_robo_managed_env(name: &str) -> bool {
    matches!(
        name,
        "ROBO_NIX_ACTIVE"
            | "ROBO_NIX_ENV_NAME"
            | "ROBO_NIX_PROMPT_PREFIX"
            | "ROBO_NIX_PARENT_ZDOTDIR"
            | "ROBO_NIX_RUNTIME_INPUT_KEY"
            | "ROBO_NIX_RUNTIME_INPUT_FILES"
            | "ROBO_NIX_COMPONENTS"
            | "ROBO_NIX_MANAGED_ENV_VARS"
            | "ROBO_NIX_PYTHON"
            | "ROBO_NIX_HOST_GRAPHICS"
            | "ROBO_NIX_LIBC_DEV"
            | "ROBO_NIX_LINUX_HEADERS"
            | "ROBO_NIX_LIBCUDA_PATH"
            | "ROBO_NIX_HOST_LIBCUDA_AUTO"
            | "ROBO_NIX_HOST_LIBCUDA_BRIDGE"
            | "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED"
            | "ROBO_NIX_HOST_GRAPHICS_LIB_DIR"
            | "ROBO_NIX_HOST_GRAPHICS_EGL_PLATFORM_DIR"
            | "ROBO_NIX_HOST_GRAPHICS_GBM_BACKEND_DIR"
            | "UV_PYTHON"
            | "UV_PYTHON_DOWNLOADS"
            | "UV_PROJECT_ENVIRONMENT"
            | "UV_CACHE_DIR"
            | "VIRTUAL_ENV"
            | "TRITON_LIBCUDA_PATH"
            | "CUDA_PATH"
            | "CUDA_HOME"
            | "CUDA_TOOLKIT_ROOT_DIR"
            | "CUDAToolkit_ROOT"
            | "CUDAHOSTCXX"
            | "CC"
            | "CXX"
            | "CPATH"
            | "C_INCLUDE_PATH"
            | "LIBRARY_PATH"
            | "LD_LIBRARY_PATH"
            | "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS"
            | "GBM_BACKENDS_PATH"
            | "__EGL_VENDOR_LIBRARY_FILENAMES"
            | "VK_ICD_FILENAMES"
            | "VK_DRIVER_FILES"
            | "__NV_PRIME_RENDER_OFFLOAD"
            | "__GLX_VENDOR_LIBRARY_NAME"
            | "__VK_LAYER_NV_optimus"
            | "NVIDIA_VISIBLE_DEVICES"
            | "WORKSPACE_ROOT"
    )
}

pub(crate) fn runtime_environment(
    config: Config,
    phase: &str,
    workspace: &Path,
    cache_key: &str,
) -> Result<Vec<(String, String)>, AppError> {
    if let Some(envs) = read_runtime_env_cache(workspace, cache_key) {
        crate::ui::output_cached_tree(
            config,
            &format!("{phase}: evaluating and realizing dev shell"),
        );
        return Ok(envs);
    }

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

pub(crate) fn cache_runtime_environment(
    workspace: &Path,
    cache_key: &str,
    envs: &[(String, String)],
) {
    let _ = write_runtime_env_cache(workspace, cache_key, envs);
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
            .map_err(|_| "runtime shell environment contains an invalid variable name")?;
        let value = String::from_utf8(entry[eq + 1..].to_vec())
            .map_err(|_| "runtime shell environment contains an invalid variable value")?;
        envs.push((name, value));
    }
    Ok(envs)
}

fn read_runtime_env_cache(workspace: &Path, cache_key: &str) -> Option<Vec<(String, String)>> {
    let bytes = fs::read(runtime_env_cache_path(workspace)).ok()?;
    let (magic, rest) = split_once_byte(&bytes, b'\n')?;
    if magic != RUNTIME_ENV_CACHE_MAGIC.as_bytes() {
        return None;
    }
    let (key, env_bytes) = split_once_byte(rest, b'\n')?;
    if key != cache_key.as_bytes() {
        return None;
    }
    let envs = parse_env_zero(env_bytes).ok()?;
    store_roots_exist(&envs).then_some(envs)
}

fn write_runtime_env_cache(
    workspace: &Path,
    cache_key: &str,
    envs: &[(String, String)],
) -> io::Result<()> {
    let state_dir = workspace.join(".robo-nix");
    fs::create_dir_all(&state_dir)?;
    let cache_path = runtime_env_cache_path(workspace);
    let tmp_path = state_dir.join(format!(
        "{RUNTIME_ENV_CACHE_FILE}.tmp-{}",
        std::process::id()
    ));
    let mut bytes = Vec::new();
    bytes.extend_from_slice(RUNTIME_ENV_CACHE_MAGIC.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(cache_key.as_bytes());
    bytes.push(b'\n');
    for (name, value) in envs {
        if name.as_bytes().contains(&b'=')
            || name.as_bytes().contains(&0)
            || value.as_bytes().contains(&0)
        {
            continue;
        }
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'=');
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }
    fs::write(&tmp_path, bytes)?;
    fs::rename(tmp_path, cache_path)
}

fn runtime_env_cache_path(workspace: &Path) -> PathBuf {
    workspace.join(".robo-nix").join(RUNTIME_ENV_CACHE_FILE)
}

fn split_once_byte(bytes: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == needle)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn store_roots_exist(envs: &[(String, String)]) -> bool {
    missing_store_roots(envs).is_empty()
}

pub(crate) fn missing_store_roots(envs: &[(String, String)]) -> Vec<PathBuf> {
    let mut paths = envs
        .iter()
        .flat_map(|(_, value)| store_roots_in_value(value))
        .filter(|path| !path.exists())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn store_roots_in_value(value: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut rest = value;
    while let Some(index) = rest.find("/nix/store/") {
        let start = &rest[index..];
        let end = start
            .char_indices()
            .find_map(|(offset, character)| (!is_store_path_character(character)).then_some(offset))
            .unwrap_or(start.len());
        let root = &start[..end];
        if root.len() > "/nix/store/".len() {
            roots.push(PathBuf::from(root));
        }
        rest = &start[end..];
    }
    roots.sort();
    roots.dedup();
    roots
}

fn is_store_path_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '/' | '.' | '_' | '+' | '-' | '?' | '=')
}

pub(crate) fn apply_env(command: &mut Command, envs: &[(String, String)]) {
    command.env_clear();
    command.envs(envs.iter().map(|(name, value)| (name, value)));
}

pub(crate) fn add_env_capture_args(command: &mut Command) {
    command.arg("sh").arg("-c").arg(ENV_CAPTURE_SCRIPT);
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostGraphicsReport {
    pub(crate) status: String,
    pub(crate) checked: Vec<String>,
    pub(crate) provider: Option<String>,
    pub(crate) linked_libraries: usize,
    pub(crate) egl_platform_dir: Option<String>,
    pub(crate) linked_egl_platforms: usize,
    pub(crate) gbm_backend_dir: Option<String>,
    pub(crate) linked_gbm_backends: usize,
    pub(crate) error: Option<String>,
    pub(crate) env_updates: Vec<String>,
}

impl HostGraphicsReport {
    pub(crate) fn decision_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("host_graphics={}", self.status)];
        if !self.checked.is_empty() {
            lines.push(format!("host_graphics_checked={}", self.checked.join(",")));
        }
        if let Some(provider) = &self.provider {
            lines.push(format!("host_graphics_provider={provider}"));
        }
        lines.push(format!(
            "host_graphics_linked_libraries={}",
            self.linked_libraries
        ));
        if let Some(dir) = &self.egl_platform_dir {
            lines.push(format!("host_graphics_egl_platform_dir={dir}"));
        }
        lines.push(format!(
            "host_graphics_egl_platforms={}",
            self.linked_egl_platforms
        ));
        if let Some(dir) = &self.gbm_backend_dir {
            lines.push(format!("host_graphics_gbm_backend_dir={dir}"));
        }
        lines.push(format!(
            "host_graphics_gbm_backends={}",
            self.linked_gbm_backends
        ));
        if let Some(error) = &self.error {
            lines.push(format!("host_graphics_error={error}"));
        }
        if !self.env_updates.is_empty() {
            lines.push(format!(
                "host_graphics_env_updates={}",
                self.env_updates.join(",")
            ));
        }
        lines
    }

    pub(crate) fn warning(&self) -> Option<String> {
        match self.status.as_str() {
            "not-selected" | "prepared" => None,
            _ => Some(format!(
                "NVIDIA host graphics provider was selected but could not be completed: {}",
                self.error.as_deref().unwrap_or(self.status.as_str())
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct HostGraphicsInputs {
    checked: Vec<String>,
    library_dirs: Vec<PathBuf>,
    library_aliases: Vec<(String, PathBuf)>,
    egl_platform_dirs: Vec<PathBuf>,
    gbm_backend_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct HostGraphicsProvider {
    lib_dir: String,
    linked_libraries: usize,
    egl_platform_dir: Option<String>,
    linked_egl_platforms: usize,
    gbm_backend_dir: Option<String>,
    linked_gbm_backends: usize,
}

pub(crate) fn append_host_nvidia_graphics_provider(
    envs: &mut Vec<(String, String)>,
    workspace: &Path,
) -> HostGraphicsReport {
    if shell_env_value(envs, "ROBO_NIX_HOST_GRAPHICS").map(String::as_str) != Some("nvidia") {
        return HostGraphicsReport {
            status: "not-selected".to_string(),
            ..HostGraphicsReport::default()
        };
    }

    let inputs = host_nvidia_provider_inputs(envs);

    match create_host_graphics_provider(workspace, &inputs) {
        Ok(provider) => {
            set_shell_env(
                envs,
                "ROBO_NIX_HOST_GRAPHICS_LIB_DIR",
                provider.lib_dir.clone(),
            );
            prepend_env_path(envs, "LD_LIBRARY_PATH", &provider.lib_dir);
            let mut env_updates = vec![
                "LD_LIBRARY_PATH".to_string(),
                "ROBO_NIX_HOST_GRAPHICS_LIB_DIR".to_string(),
            ];
            if let Some(egl_platform_dir) = &provider.egl_platform_dir {
                set_shell_env(
                    envs,
                    "ROBO_NIX_HOST_GRAPHICS_EGL_PLATFORM_DIR",
                    egl_platform_dir.clone(),
                );
                prepend_env_path(
                    envs,
                    "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
                    egl_platform_dir,
                );
                env_updates.extend([
                    "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS".to_string(),
                    "ROBO_NIX_HOST_GRAPHICS_EGL_PLATFORM_DIR".to_string(),
                ]);
            }
            if let Some(gbm_backend_dir) = &provider.gbm_backend_dir {
                set_shell_env(
                    envs,
                    "ROBO_NIX_HOST_GRAPHICS_GBM_BACKEND_DIR",
                    gbm_backend_dir.clone(),
                );
                prepend_env_path(envs, "GBM_BACKENDS_PATH", gbm_backend_dir);
                env_updates.extend([
                    "GBM_BACKENDS_PATH".to_string(),
                    "ROBO_NIX_HOST_GRAPHICS_GBM_BACKEND_DIR".to_string(),
                ]);
            }
            HostGraphicsReport {
                status: "prepared".to_string(),
                checked: inputs.checked,
                provider: Some(provider.lib_dir),
                linked_libraries: provider.linked_libraries,
                egl_platform_dir: provider.egl_platform_dir,
                linked_egl_platforms: provider.linked_egl_platforms,
                gbm_backend_dir: provider.gbm_backend_dir,
                linked_gbm_backends: provider.linked_gbm_backends,
                env_updates,
                ..HostGraphicsReport::default()
            }
        }
        Err(error) => HostGraphicsReport {
            status: "provider-error".to_string(),
            checked: inputs.checked,
            error: Some(error.message().to_string()),
            ..HostGraphicsReport::default()
        },
    }
}

fn host_nvidia_provider_inputs(envs: &[(String, String)]) -> HostGraphicsInputs {
    let mut inputs = HostGraphicsInputs::default();

    if let Some(dir) = shell_env_value(envs, "ROBO_NIX_NVIDIA_DRIVER_LIB_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("ROBO_NIX_NVIDIA_DRIVER_LIB_DIR").map(PathBuf::from))
    {
        inputs
            .checked
            .push(format!("ROBO_NIX_NVIDIA_DRIVER_LIB_DIR={}", dir.display()));
        push_existing_dir(&mut inputs.gbm_backend_dirs, dir.clone());
        push_existing_dir(&mut inputs.library_dirs, dir);
    }

    inputs.checked.push("ldconfig -p".to_string());
    let ldconfig_names = REQUIRED_NVIDIA_GRAPHICS_LIBRARIES
        .iter()
        .chain(COMMON_NVIDIA_GRAPHICS_LIBRARIES.iter())
        .copied()
        .collect::<Vec<_>>();
    for (name, path) in find_libraries_with_ldconfig(&ldconfig_names) {
        if let Some(parent) = path.parent() {
            push_existing_dir(&mut inputs.library_dirs, parent.to_path_buf());
        }
        push_library_alias(&mut inputs.library_aliases, name, path);
    }

    for dir in KNOWN_HOST_NVIDIA_LIBRARY_DIRS {
        inputs.checked.push((*dir).to_string());
        push_existing_dir(&mut inputs.library_dirs, PathBuf::from(dir));
    }

    for dir in KNOWN_HOST_NVIDIA_EGL_PLATFORM_DIRS {
        inputs.checked.push((*dir).to_string());
        push_existing_dir(&mut inputs.egl_platform_dirs, PathBuf::from(dir));
    }
    if let Some(paths) = shell_env_value(envs, "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS")
        .map(std::ffi::OsString::from)
        .or_else(|| env::var_os("__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS"))
    {
        inputs
            .checked
            .push("__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS".to_string());
        for dir in env::split_paths(&paths) {
            push_existing_dir(&mut inputs.egl_platform_dirs, dir);
        }
    }
    for dir in KNOWN_HOST_NVIDIA_GBM_BACKEND_DIRS {
        inputs.checked.push((*dir).to_string());
        push_existing_dir(&mut inputs.gbm_backend_dirs, PathBuf::from(dir));
    }
    if let Some(paths) = shell_env_value(envs, "GBM_BACKENDS_PATH")
        .map(std::ffi::OsString::from)
        .or_else(|| env::var_os("GBM_BACKENDS_PATH"))
    {
        inputs.checked.push("GBM_BACKENDS_PATH".to_string());
        for dir in env::split_paths(&paths) {
            push_existing_dir(&mut inputs.gbm_backend_dirs, dir);
        }
    }

    inputs
}

fn push_existing_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dir.is_dir() || dirs.iter().any(|candidate| candidate == &dir) {
        return;
    }
    dirs.push(dir);
}

fn push_library_alias(aliases: &mut Vec<(String, PathBuf)>, name: String, path: PathBuf) {
    if !path.is_file() || aliases.iter().any(|(candidate, _)| candidate == &name) {
        return;
    }
    aliases.push((name, path));
}

fn create_host_graphics_provider(
    workspace: &Path,
    inputs: &HostGraphicsInputs,
) -> Result<HostGraphicsProvider, AppError> {
    with_project_lock(workspace, "host-graphics", || {
        let provider_dir = workspace.join(".robo-nix").join("host-graphics");
        let lib_dir = provider_dir.join("lib");
        let egl_platform_dir = provider_dir.join("egl_external_platform.d");
        let gbm_backend_dir = provider_dir.join("gbm");
        fs::create_dir_all(&lib_dir).map_err(|err| {
            AppError::project(format!(
                "failed to create host graphics bridge directory: {err}"
            ))
        })?;
        fs::create_dir_all(&egl_platform_dir).map_err(|err| {
            AppError::project(format!(
                "failed to create host graphics EGL platform directory: {err}"
            ))
        })?;
        fs::create_dir_all(&gbm_backend_dir).map_err(|err| {
            AppError::project(format!(
                "failed to create host graphics GBM backend directory: {err}"
            ))
        })?;
        clean_robo_owned_dir(&lib_dir, "host graphics bridge")?;
        clean_robo_owned_dir(&egl_platform_dir, "host graphics EGL platform")?;
        clean_robo_owned_dir(&gbm_backend_dir, "host graphics GBM backend")?;

        let mut linked_libraries = link_nvidia_driver_libraries(&inputs.library_dirs, &lib_dir)?;
        linked_libraries += link_nvidia_library_aliases(&inputs.library_aliases, &lib_dir)?;
        let missing = REQUIRED_NVIDIA_GRAPHICS_LIBRARIES
            .iter()
            .filter(|name| !lib_dir.join(name).exists())
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(AppError::project(format!(
                "missing required NVIDIA graphics libraries: {}",
                missing.join(", ")
            )));
        }

        let linked_egl_platforms =
            link_nvidia_egl_platform_configs(&inputs.egl_platform_dirs, &egl_platform_dir)?;
        let linked_gbm_backends =
            link_nvidia_gbm_backends(&inputs.gbm_backend_dirs, &gbm_backend_dir)?;
        Ok(HostGraphicsProvider {
            lib_dir: lib_dir.display().to_string(),
            linked_libraries,
            egl_platform_dir: (linked_egl_platforms > 0)
                .then(|| egl_platform_dir.display().to_string()),
            linked_egl_platforms,
            gbm_backend_dir: (linked_gbm_backends > 0)
                .then(|| gbm_backend_dir.display().to_string()),
            linked_gbm_backends,
        })
    })
}

fn clean_robo_owned_dir(dir: &Path, label: &str) -> Result<(), AppError> {
    for entry in fs::read_dir(dir).map_err(|err| {
        AppError::project(format!(
            "failed to inspect {label} directory {}: {err}",
            dir.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            AppError::project(format!(
                "failed to inspect {label} directory {}: {err}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|err| {
            AppError::project(format!(
                "failed to inspect {label} entry {}: {err}",
                path.display()
            ))
        })?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&path).map_err(|err| {
                AppError::project(format!(
                    "failed to clean {label} directory {}: {err}",
                    path.display()
                ))
            })?;
        } else {
            fs::remove_file(&path).map_err(|err| {
                AppError::project(format!(
                    "failed to clean {label} entry {}: {err}",
                    path.display()
                ))
            })?;
        }
    }
    Ok(())
}

fn link_nvidia_driver_libraries(
    driver_dirs: &[PathBuf],
    lib_dir: &Path,
) -> Result<usize, AppError> {
    let mut linked = 0;
    let mut linked_names = BTreeSet::new();
    for driver_dir in driver_dirs {
        let mut entries = fs::read_dir(driver_dir)
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host NVIDIA driver directory {}: {err}",
                    driver_dir.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host NVIDIA driver directory {}: {err}",
                    driver_dir.display()
                ))
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_name = entry.file_name();
            let file_name_text = file_name.to_string_lossy();
            if !is_nvidia_driver_library_name(&file_name_text) {
                continue;
            }
            let source = entry.path();
            if !source.is_file() {
                continue;
            }
            if !linked_names.insert(file_name_text.to_string()) {
                continue;
            }
            let link = lib_dir.join(&file_name);
            replace_file_link(&source, &link).map_err(|err| {
                AppError::project(format!(
                    "failed to link host NVIDIA driver library into {}: {err}",
                    link.display()
                ))
            })?;
            linked += 1;
        }
    }
    Ok(linked)
}

fn link_nvidia_library_aliases(
    aliases: &[(String, PathBuf)],
    lib_dir: &Path,
) -> Result<usize, AppError> {
    let mut linked = 0;
    for (name, source) in aliases {
        if !is_nvidia_driver_library_name(name) {
            continue;
        }
        let link = lib_dir.join(name);
        if link.exists() || fs::symlink_metadata(&link).is_ok() {
            continue;
        }
        replace_file_link(source, &link).map_err(|err| {
            AppError::project(format!(
                "failed to link host NVIDIA driver library into {}: {err}",
                link.display()
            ))
        })?;
        linked += 1;
    }
    Ok(linked)
}

fn link_nvidia_egl_platform_configs(
    config_dirs: &[PathBuf],
    provider_dir: &Path,
) -> Result<usize, AppError> {
    let mut linked = 0;
    let mut linked_names = BTreeSet::new();
    for config_dir in config_dirs {
        let mut entries = fs::read_dir(config_dir)
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host EGL platform directory {}: {err}",
                    config_dir.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host EGL platform directory {}: {err}",
                    config_dir.display()
                ))
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_name = entry.file_name();
            let file_name_text = file_name.to_string_lossy();
            if !is_nvidia_egl_platform_config_name(&file_name_text) {
                continue;
            }
            let source = entry.path();
            if !source.is_file() {
                continue;
            }
            if !linked_names.insert(file_name_text.to_string()) {
                continue;
            }
            let link = provider_dir.join(&file_name);
            replace_file_link(&source, &link).map_err(|err| {
                AppError::project(format!(
                    "failed to link host NVIDIA EGL platform config into {}: {err}",
                    link.display()
                ))
            })?;
            linked += 1;
        }
    }
    Ok(linked)
}

fn link_nvidia_gbm_backends(
    backend_dirs: &[PathBuf],
    provider_dir: &Path,
) -> Result<usize, AppError> {
    let mut linked = 0;
    let mut linked_names = BTreeSet::new();
    for backend_dir in backend_dirs {
        let mut entries = fs::read_dir(backend_dir)
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host GBM backend directory {}: {err}",
                    backend_dir.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                AppError::project(format!(
                    "failed to inspect host GBM backend directory {}: {err}",
                    backend_dir.display()
                ))
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_name = entry.file_name();
            let file_name_text = file_name.to_string_lossy();
            if !is_nvidia_gbm_backend_name(&file_name_text) {
                continue;
            }
            let source = entry.path();
            if !source.is_file() {
                continue;
            }
            if !linked_names.insert(file_name_text.to_string()) {
                continue;
            }
            let link = provider_dir.join(&file_name);
            replace_file_link(&source, &link).map_err(|err| {
                AppError::project(format!(
                    "failed to link host NVIDIA GBM backend into {}: {err}",
                    link.display()
                ))
            })?;
            linked += 1;
        }
    }
    Ok(linked)
}

fn find_libraries_with_ldconfig(names: &[&str]) -> Vec<(String, PathBuf)> {
    let output = match Command::new("ldconfig").arg("-p").output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let mut found_names = BTreeSet::new();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let found_name = line.split_whitespace().next()?;
            if !names.contains(&found_name) || !found_names.insert(found_name.to_string()) {
                return None;
            }
            line.rsplit_once(" => ")
                .map(|(_, path)| (found_name.to_string(), PathBuf::from(path.trim())))
                .filter(|(_, path)| path.is_file())
        })
        .collect()
}

fn is_nvidia_egl_platform_config_name(name: &str) -> bool {
    name.ends_with(".json") && name.to_ascii_lowercase().contains("nvidia")
}

fn is_nvidia_gbm_backend_name(name: &str) -> bool {
    name == "nvidia-drm_gbm.so"
        || (name.ends_with(".so") && name.to_ascii_lowercase().contains("nvidia"))
}

fn is_nvidia_driver_library_name(name: &str) -> bool {
    NVIDIA_DRIVER_LIBRARY_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        && name.contains(".so")
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostCudaReport {
    pub(crate) status: String,
    pub(crate) needed_by: Vec<String>,
    pub(crate) checked: Vec<String>,
    pub(crate) source: Option<String>,
    pub(crate) libcuda: Option<String>,
    pub(crate) driver_version: Option<String>,
    pub(crate) bridge: Option<String>,
    pub(crate) bridge_error: Option<String>,
    pub(crate) env_updates: Vec<String>,
}

impl HostCudaReport {
    pub(crate) fn decision_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("host_cuda={}", self.status)];
        if !self.needed_by.is_empty() {
            lines.push(format!("host_cuda_needed_by={}", self.needed_by.join(",")));
        }
        if !self.checked.is_empty() {
            lines.push(format!("host_cuda_checked={}", self.checked.join(",")));
        }
        if let Some(source) = &self.source {
            lines.push(format!("host_cuda_source={source}"));
        }
        if let Some(libcuda) = &self.libcuda {
            lines.push(format!("host_cuda_libcuda={libcuda}"));
        }
        if let Some(version) = &self.driver_version {
            lines.push(format!("host_nvidia_driver={version}"));
        }
        if let Some(bridge) = &self.bridge {
            lines.push(format!("host_cuda_bridge={bridge}"));
        }
        if let Some(error) = &self.bridge_error {
            lines.push(format!("host_cuda_bridge_error={error}"));
        }
        if !self.env_updates.is_empty() {
            lines.push(format!(
                "host_cuda_env_updates={}",
                self.env_updates.join(",")
            ));
        }
        lines
    }
}

#[derive(Debug, Clone)]
struct FoundLibcuda {
    source: String,
    path: String,
}

#[derive(Debug, Clone, Default)]
struct HostLibcudaProbe {
    checked: Vec<String>,
    found: Option<FoundLibcuda>,
}

#[derive(Debug, Clone, Default)]
struct HostCudaBridgeResult {
    bridge: Option<String>,
    bridge_error: Option<String>,
    env_updates: Vec<String>,
}

pub(crate) fn append_host_cuda_driver_bridge(
    envs: &mut Vec<(String, String)>,
    workspace: &Path,
) -> HostCudaReport {
    let needed_by = host_cuda_need_reasons(workspace);
    if needed_by.is_empty() {
        return HostCudaReport {
            status: "not-needed".to_string(),
            ..HostCudaReport::default()
        };
    }
    let driver_version = probe_nvidia_driver_version();
    if env_flag_enabled("ROBO_NIX_DISABLE_HOST_CUDA_AUTO") {
        return HostCudaReport {
            status: "disabled".to_string(),
            needed_by,
            driver_version,
            ..HostCudaReport::default()
        };
    }
    if shell_env_value(envs, "ROBO_NIX_LIBCUDA_PATH").is_some()
        || env::var_os("ROBO_NIX_LIBCUDA_PATH").is_some()
    {
        return HostCudaReport {
            status: "explicit".to_string(),
            needed_by,
            checked: vec!["ROBO_NIX_LIBCUDA_PATH".to_string()],
            source: Some("ROBO_NIX_LIBCUDA_PATH".to_string()),
            driver_version,
            ..HostCudaReport::default()
        };
    }

    let probe = find_host_libcuda(envs);
    let Some(found) = probe.found else {
        return HostCudaReport {
            status: "needed-missing".to_string(),
            needed_by,
            checked: probe.checked,
            driver_version,
            ..HostCudaReport::default()
        };
    };

    let bridge = apply_host_cuda_driver_bridge(envs, workspace, &found.path);
    HostCudaReport {
        status: "auto-found".to_string(),
        needed_by,
        checked: probe.checked,
        source: Some(found.source),
        libcuda: Some(found.path),
        driver_version,
        bridge: bridge.bridge,
        bridge_error: bridge.bridge_error,
        env_updates: bridge.env_updates,
    }
}

fn host_cuda_need_reasons(workspace: &Path) -> Vec<String> {
    let mut reasons = Vec::new();
    if let Ok(dependencies) = dependency_names_from_pyproject(workspace) {
        for dependency in dependencies {
            if HOST_CUDA_DEPENDENCIES.contains(&dependency.as_str()) {
                reasons.push(format!("pyproject:{dependency}"));
            }
        }
    }
    reasons.extend(
        uv_lock_cuda_packages(workspace)
            .into_iter()
            .map(|package| format!("uv.lock:{package}")),
    );
    reasons.sort();
    reasons.dedup();
    reasons
}

fn uv_lock_cuda_packages(workspace: &Path) -> Vec<String> {
    fs::read_to_string(workspace.join("uv.lock"))
        .map(|text| uv_lock_text_cuda_packages(&text))
        .unwrap_or_default()
}

fn uv_lock_text_cuda_packages(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("name = "))
        .filter_map(extract_quoted)
        .filter(|name| cuda_lock_package_needs_host_driver(name))
        .map(str::to_string)
        .collect()
}

fn cuda_lock_package_needs_host_driver(name: &str) -> bool {
    let name = normalize_package_name(name);
    HOST_CUDA_DEPENDENCIES.contains(&name.as_str())
        || (name.starts_with("nvidia-") && (name.contains("-cu11") || name.contains("-cu12")))
}

fn find_host_libcuda(envs: &[(String, String)]) -> HostLibcudaProbe {
    let mut probe = HostLibcudaProbe::default();
    probe.checked.push("ROBO_NIX_LIBCUDA_PATH".to_string());

    probe.checked.push("LD_LIBRARY_PATH".to_string());
    let captured_ld_library_path = shell_env_value(envs, "LD_LIBRARY_PATH").map(String::as_str);
    if let Some(path) = captured_ld_library_path
        .map(std::ffi::OsString::from)
        .or_else(|| env::var_os("LD_LIBRARY_PATH"))
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .find_map(|dir| find_libcuda_in_dir(&dir))
        .map(|path| path.display().to_string())
    {
        probe.found = Some(FoundLibcuda {
            source: "LD_LIBRARY_PATH".to_string(),
            path,
        });
        return probe;
    }

    probe.checked.push("ldconfig -p".to_string());
    if let Some(path) = find_libcuda_with_ldconfig() {
        probe.found = Some(FoundLibcuda {
            source: "ldconfig -p".to_string(),
            path,
        });
        return probe;
    }

    for dir in KNOWN_HOST_LIBCUDA_DIRS {
        probe.checked.push((*dir).to_string());
        if let Some(path) = find_libcuda_in_dir(Path::new(dir)) {
            probe.found = Some(FoundLibcuda {
                source: (*dir).to_string(),
                path: path.display().to_string(),
            });
            return probe;
        }
    }

    probe
}

fn find_libcuda_with_ldconfig() -> Option<String> {
    find_library_with_ldconfig_names(LIBCUDA_NAMES)
}

fn find_library_with_ldconfig_names(names: &[&str]) -> Option<String> {
    let output = Command::new("ldconfig").arg("-p").output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let Some(found_name) = line.split_whitespace().next() else {
                return None;
            };
            if !names.contains(&found_name) {
                return None;
            }
            line.rsplit_once(" => ")
                .map(|(_, path)| path.trim().to_string())
                .filter(|path| Path::new(path).is_file())
        })
}

fn find_libcuda_in_dir(dir: &Path) -> Option<PathBuf> {
    LIBCUDA_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn apply_host_cuda_driver_bridge(
    envs: &mut Vec<(String, String)>,
    workspace: &Path,
    libcuda: &str,
) -> HostCudaBridgeResult {
    let mut result = HostCudaBridgeResult::default();
    let Some(driver_dir) = Path::new(libcuda).parent() else {
        result.bridge_error = Some("libcuda path has no parent directory".to_string());
        return result;
    };
    let driver_dir_path = driver_dir.to_path_buf();
    let driver_dir = driver_dir.display().to_string();

    set_shell_env(envs, "ROBO_NIX_LIBCUDA_PATH", libcuda.to_string());
    result.env_updates.push("ROBO_NIX_LIBCUDA_PATH".to_string());
    set_shell_env(envs, "ROBO_NIX_HOST_LIBCUDA_AUTO", driver_dir.clone());
    result
        .env_updates
        .push("ROBO_NIX_HOST_LIBCUDA_AUTO".to_string());

    if shell_env_value(envs, "TRITON_LIBCUDA_PATH").is_none()
        && env::var_os("TRITON_LIBCUDA_PATH").is_none()
    {
        set_shell_env(envs, "TRITON_LIBCUDA_PATH", driver_dir.clone());
        result.env_updates.push("TRITON_LIBCUDA_PATH".to_string());
    }

    if driver_dir_contains_glibc(&driver_dir_path) {
        match create_libcuda_bridge(workspace, libcuda) {
            Ok(bridge_dir) => {
                set_shell_env(envs, "ROBO_NIX_HOST_LIBCUDA_BRIDGE", bridge_dir.clone());
                result
                    .env_updates
                    .push("ROBO_NIX_HOST_LIBCUDA_BRIDGE".to_string());
                append_ld_library_path(envs, &bridge_dir);
                result.env_updates.push("LD_LIBRARY_PATH".to_string());
                result.bridge = Some(bridge_dir);
            }
            Err(err) => {
                result.bridge_error = Some(err.message().to_string());
            }
        }
        set_shell_env(
            envs,
            "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED",
            driver_dir,
        );
        result
            .env_updates
            .push("ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED".to_string());
        return result;
    }

    append_ld_library_path(envs, &driver_dir);
    result.env_updates.push("LD_LIBRARY_PATH".to_string());
    result
}

fn driver_dir_contains_glibc(driver_dir: &Path) -> bool {
    driver_dir.join("libc.so.6").exists() || driver_dir.join("ld-linux-x86-64.so.2").exists()
}

fn create_libcuda_bridge(workspace: &Path, libcuda: &str) -> Result<String, AppError> {
    with_project_lock(workspace, "host-libs", || {
        let bridge_dir = workspace.join(".robo-nix").join("host-libs");
        fs::create_dir_all(&bridge_dir).map_err(|err| {
            AppError::project(format!(
                "failed to create host CUDA bridge directory: {err}"
            ))
        })?;
        for name in LIBCUDA_NAMES {
            let link = bridge_dir.join(name);
            let _ = fs::remove_file(&link);
            replace_file_link(Path::new(libcuda), &link).map_err(|err| {
                AppError::project(format!(
                    "failed to link host CUDA driver library into {}: {err}",
                    link.display()
                ))
            })?;
        }
        Ok(bridge_dir.display().to_string())
    })
}

#[cfg(unix)]
fn replace_file_link(source: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, link)
}

#[cfg(not(unix))]
fn replace_file_link(source: &Path, link: &Path) -> std::io::Result<()> {
    fs::copy(source, link).map(|_| ())
}

fn append_ld_library_path(envs: &mut Vec<(String, String)>, path: &str) {
    let library_path = shell_env_value(envs, "LD_LIBRARY_PATH")
        .cloned()
        .unwrap_or_default();
    if path_list_contains(&library_path, path) {
        return;
    }
    let value = if library_path.is_empty() {
        path.to_string()
    } else {
        format!("{library_path}:{path}")
    };
    set_shell_env(envs, "LD_LIBRARY_PATH", value);
}

fn prepend_env_path(envs: &mut Vec<(String, String)>, name: &str, path: &str) {
    let current = shell_env_value(envs, name).cloned().unwrap_or_default();
    if path_list_contains(&current, path) {
        return;
    }
    let value = if current.is_empty() {
        path.to_string()
    } else {
        format!("{path}:{current}")
    };
    set_shell_env(envs, name, value);
}

fn shell_env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a String> {
    envs.iter()
        .find_map(|(candidate, value)| (candidate == name).then_some(value))
}

fn set_shell_env(envs: &mut Vec<(String, String)>, name: &str, value: String) {
    envs.retain(|(candidate, _)| candidate != name);
    envs.push((name.to_string(), value));
}

fn path_list_contains(paths: &str, needle: &str) -> bool {
    paths.split(':').any(|path| path == needle)
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn probe_nvidia_driver_version() -> Option<String> {
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=driver_version")
        .arg("--format=csv,noheader")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

pub(crate) fn with_project_lock<T, F>(
    workspace: &Path,
    name: &str,
    action: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError>,
{
    let state_dir = workspace.join(".robo-nix");
    fs::create_dir_all(&state_dir)
        .map_err(|err| AppError::project(format!("failed to create .robo-nix/: {err}")))?;
    let lock_path = state_dir.join(format!("{name}.lock"));
    let timeout = project_lock_timeout();
    let start = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                if let Err(err) = writeln!(file, "pid={}\nname={name}", std::process::id()) {
                    let _ = fs::remove_file(&lock_path);
                    return Err(AppError::project(format!(
                        "failed to write {}: {err}",
                        lock_path.display()
                    )));
                }
                let _guard = ProjectLockGuard { path: lock_path };
                return action();
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if start.elapsed() >= timeout {
                    return Err(AppError::project(format!(
                        "timed out waiting for robo project lock {}",
                        lock_path.display()
                    ))
                    .with_hint(format!(
                        "another robo process may be preparing this project; remove {} only if no robo process is active.",
                        lock_path.display()
                    )));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(AppError::project(format!(
                    "failed to create robo project lock {}: {err}",
                    lock_path.display()
                )));
            }
        }
    }
}

fn project_lock_timeout() -> Duration {
    env::var("ROBO_NIX_LOCK_TIMEOUT")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_LOCK_TIMEOUT_SECONDS))
}

struct ProjectLockGuard {
    path: PathBuf,
}

impl Drop for ProjectLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn extract_quoted(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    value.split_once('"').map(|(quoted, _)| quoted)
}

fn normalize_package_name(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            '_' | '.' => '-',
            other => other.to_ascii_lowercase(),
        })
        .collect()
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

    #[test]
    fn runtime_env_cache_round_trips_nul_environment() {
        let root = temp_project("runtime-env-cache");
        fs::create_dir_all(root.join(".robo-nix")).unwrap();
        let envs = vec![
            ("PATH".to_string(), "/bin".to_string()),
            (
                "ROBO_NIX_COMPONENTS".to_string(),
                "native-build".to_string(),
            ),
        ];

        write_runtime_env_cache(&root, "cache-key", &envs).unwrap();

        assert_eq!(read_runtime_env_cache(&root, "cache-key"), Some(envs));
        assert_eq!(read_runtime_env_cache(&root, "other-key"), None);

        cleanup(root);
    }

    #[test]
    fn store_paths_are_extracted_from_env_values() {
        assert_eq!(
            store_roots_in_value("/nix/store/abc-package/lib:/other"),
            vec![PathBuf::from("/nix/store/abc-package/lib")]
        );
    }

    #[test]
    fn apply_env_clears_values_absent_from_captured_environment() {
        let path = std::env::var("PATH").unwrap_or_else(|_| "/bin:/usr/bin".to_string());
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("printf '%s:%s' \"${ROBO_NIX_TEST_LEAK-unset}\" \"${KEEP_ME-unset}\"")
            .env("ROBO_NIX_TEST_LEAK", "leak");

        apply_env(
            &mut command,
            &[
                ("PATH".to_string(), path),
                ("KEEP_ME".to_string(), "1".to_string()),
            ],
        );

        let output = command.output().expect("test shell should run");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset:1");
    }

    #[test]
    fn cuda_lock_packages_imply_host_driver_requirement() {
        assert_eq!(
            uv_lock_text_cuda_packages(
                r#"
[[package]]
name = "nvidia-cuda-runtime-cu12"
version = "12.8.0"
"#
            ),
            vec!["nvidia-cuda-runtime-cu12".to_string()]
        );
        assert!(uv_lock_text_cuda_packages(
            r#"
[[package]]
name = "numpy"
version = "2.0.0"
"#
        )
        .is_empty());
    }

    #[test]
    fn host_cuda_driver_bridge_sets_minimal_runtime_vars() {
        let root = temp_project("host-cuda-minimal");
        fs::create_dir_all(&root).unwrap();
        let mut env = vec![("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string())];

        apply_host_cuda_driver_bridge(&mut env, &root, "/run/opengl-driver/lib/libcuda.so.1");

        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_LIBCUDA_PATH").map(String::as_str),
            Some("/run/opengl-driver/lib/libcuda.so.1")
        );
        assert_eq!(
            shell_env_value(&env, "TRITON_LIBCUDA_PATH").map(String::as_str),
            Some("/run/opengl-driver/lib")
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_LIBCUDA_AUTO").map(String::as_str),
            Some("/run/opengl-driver/lib")
        );
        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some("/nix/store/lib:/run/opengl-driver/lib")
        );

        cleanup(root);
    }

    #[test]
    fn host_cuda_driver_bridge_does_not_inject_glibc_dir() {
        let root = temp_project("host-cuda-glibc");
        let driver_dir = root.join("driver");
        fs::create_dir_all(&driver_dir).unwrap();
        fs::write(driver_dir.join("libc.so.6"), b"").unwrap();
        fs::write(driver_dir.join("libcuda.so.1"), b"").unwrap();

        let mut env = vec![("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string())];
        apply_host_cuda_driver_bridge(
            &mut env,
            &root,
            &driver_dir.join("libcuda.so.1").display().to_string(),
        );

        let bridge_dir = root.join(".robo-nix").join("host-libs");
        let bridge_dir_text = bridge_dir.display().to_string();
        let expected_library_path = format!("/nix/store/lib:{bridge_dir_text}");
        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some(expected_library_path.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_LIBCUDA_BRIDGE").map(String::as_str),
            Some(bridge_dir_text.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED")
                .map(String::as_str),
            Some(driver_dir.to_str().unwrap())
        );
        assert!(fs::symlink_metadata(bridge_dir.join("libcuda.so.1")).is_ok());
        assert!(fs::symlink_metadata(bridge_dir.join("libcuda.so")).is_ok());

        cleanup(root);
    }

    #[test]
    fn host_graphics_provider_links_nvidia_libs_without_host_system_libs() {
        let root = temp_project("host-graphics-provider");
        let driver_dir = root.join("driver");
        let egl_platform_source = root.join("egl-platforms");
        let gbm_backend_source = root.join("gbm");
        fs::create_dir_all(&driver_dir).unwrap();
        fs::create_dir_all(&egl_platform_source).unwrap();
        fs::create_dir_all(&gbm_backend_source).unwrap();
        fs::write(driver_dir.join("libGLX_nvidia.so.0"), b"").unwrap();
        fs::write(driver_dir.join("libEGL_nvidia.so.0"), b"").unwrap();
        fs::write(driver_dir.join("libnvidia-glcore.so.555.1"), b"").unwrap();
        fs::write(driver_dir.join("libcuda.so.1"), b"").unwrap();
        fs::write(driver_dir.join("libc.so.6"), b"").unwrap();
        fs::write(egl_platform_source.join("15_nvidia_gbm.json"), b"{}").unwrap();
        fs::write(egl_platform_source.join("50_mesa.json"), b"{}").unwrap();
        fs::write(gbm_backend_source.join("nvidia-drm_gbm.so"), b"").unwrap();
        fs::write(gbm_backend_source.join("kms_swrast_dri.so"), b"").unwrap();
        fs::create_dir_all(root.join(".robo-nix/host-graphics/lib")).unwrap();
        fs::create_dir_all(root.join(".robo-nix/host-graphics/egl_external_platform.d")).unwrap();
        fs::create_dir_all(root.join(".robo-nix/host-graphics/gbm")).unwrap();
        fs::write(
            root.join(".robo-nix/host-graphics/lib/libcuda.so.1"),
            b"stale",
        )
        .unwrap();
        fs::write(root.join(".robo-nix/host-graphics/lib/libc.so.6"), b"stale").unwrap();
        fs::write(
            root.join(".robo-nix/host-graphics/egl_external_platform.d/50_mesa.json"),
            b"stale",
        )
        .unwrap();
        let mut env = vec![
            ("ROBO_NIX_HOST_GRAPHICS".to_string(), "nvidia".to_string()),
            (
                "VK_ICD_FILENAMES".to_string(),
                "/host/nvidia_icd.json".to_string(),
            ),
            (
                "ROBO_NIX_NVIDIA_DRIVER_LIB_DIR".to_string(),
                driver_dir.display().to_string(),
            ),
            (
                "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS".to_string(),
                egl_platform_source.display().to_string(),
            ),
            (
                "GBM_BACKENDS_PATH".to_string(),
                gbm_backend_source.display().to_string(),
            ),
            ("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string()),
        ];

        let report = append_host_nvidia_graphics_provider(&mut env, &root);

        assert_eq!(report.status, "prepared");
        let provider_lib = root.join(".robo-nix/host-graphics/lib");
        let provider_lib_text = provider_lib.display().to_string();
        let provider_egl_platform = root.join(".robo-nix/host-graphics/egl_external_platform.d");
        let provider_egl_platform_text = provider_egl_platform.display().to_string();
        let provider_gbm_backend = root.join(".robo-nix/host-graphics/gbm");
        let provider_gbm_backend_text = provider_gbm_backend.display().to_string();
        let expected_library_path = format!("{provider_lib_text}:/nix/store/lib");
        let expected_egl_platform_path = format!(
            "{}:{}",
            provider_egl_platform_text,
            egl_platform_source.display()
        );
        let expected_gbm_backend_path = format!(
            "{}:{}",
            provider_gbm_backend_text,
            gbm_backend_source.display()
        );
        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some(expected_library_path.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS").map(String::as_str),
            Some(expected_egl_platform_path.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "GBM_BACKENDS_PATH").map(String::as_str),
            Some(expected_gbm_backend_path.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "VK_ICD_FILENAMES").map(String::as_str),
            Some("/host/nvidia_icd.json")
        );
        assert!(report.linked_egl_platforms >= 1);
        assert!(report.linked_gbm_backends >= 1);
        assert!(fs::symlink_metadata(provider_lib.join("libGLX_nvidia.so.0")).is_ok());
        assert!(fs::symlink_metadata(provider_lib.join("libEGL_nvidia.so.0")).is_ok());
        assert!(fs::symlink_metadata(provider_lib.join("libnvidia-glcore.so.555.1")).is_ok());
        assert!(fs::symlink_metadata(provider_lib.join("libcuda.so.1")).is_err());
        assert!(fs::symlink_metadata(provider_lib.join("libc.so.6")).is_err());
        assert!(fs::symlink_metadata(provider_egl_platform.join("15_nvidia_gbm.json")).is_ok());
        assert!(fs::symlink_metadata(provider_egl_platform.join("50_mesa.json")).is_err());
        assert!(fs::symlink_metadata(provider_gbm_backend.join("nvidia-drm_gbm.so")).is_ok());
        assert!(fs::symlink_metadata(provider_gbm_backend.join("kms_swrast_dri.so")).is_err());

        cleanup(root);
    }

    #[test]
    fn host_cuda_driver_bridge_does_not_duplicate_library_path() {
        let root = temp_project("host-cuda-dedup");
        fs::create_dir_all(&root).unwrap();
        let mut env = vec![
            (
                "LD_LIBRARY_PATH".to_string(),
                "/nix/store/lib:/run/opengl-driver/lib".to_string(),
            ),
            (
                "TRITON_LIBCUDA_PATH".to_string(),
                "/custom/triton".to_string(),
            ),
        ];

        apply_host_cuda_driver_bridge(&mut env, &root, "/run/opengl-driver/lib/libcuda.so.1");

        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some("/nix/store/lib:/run/opengl-driver/lib")
        );
        assert_eq!(
            shell_env_value(&env, "TRITON_LIBCUDA_PATH").map(String::as_str),
            Some("/custom/triton")
        );

        cleanup(root);
    }

    #[test]
    fn host_cuda_probe_uses_captured_ld_library_path() {
        let root = temp_project("host-cuda-captured-ld");
        let driver_dir = root.join("driver");
        fs::create_dir_all(&driver_dir).unwrap();
        fs::write(driver_dir.join("libcuda.so.1"), b"").unwrap();
        let env = vec![(
            "LD_LIBRARY_PATH".to_string(),
            driver_dir.display().to_string(),
        )];

        let probe = find_host_libcuda(&env);

        let found = probe.found.unwrap();
        assert_eq!(found.source, "LD_LIBRARY_PATH");
        assert_eq!(
            found.path,
            driver_dir.join("libcuda.so.1").display().to_string()
        );

        cleanup(root);
    }

    #[test]
    fn public_env_vars_are_documented() {
        let docs = [
            include_str!("../README.md"),
            include_str!("../docs/users/getting-started.md"),
            include_str!("../docs/users/runtime.md"),
            include_str!("../docs/users/troubleshooting.md"),
            include_str!("../docs/developers/overview.md"),
            include_str!("../docs/developers/cli-ux.md"),
        ]
        .join("\n");

        for spec in ENV_VARS
            .iter()
            .filter(|spec| spec.visibility == EnvVarVisibility::Public)
        {
            assert!(
                docs.contains(spec.name),
                "public env var {} is missing from docs",
                spec.name
            );
            assert!(
                !spec.description.trim().is_empty(),
                "env var {} must have a description",
                spec.name
            );
        }
    }

    #[test]
    fn optional_cuda_dependency_requires_host_driver() {
        let root = temp_project("host-cuda-pyproject");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = []

[project.optional-dependencies]
gpu = ["cupy-cuda12x"]
"#,
        )
        .unwrap();

        assert_eq!(
            host_cuda_need_reasons(&root),
            vec!["pyproject:cupy-cuda12x".to_string()]
        );

        cleanup(root);
    }

    #[test]
    fn project_lock_times_out_when_lock_is_held() {
        let root = temp_project("lock-timeout");
        fs::create_dir_all(root.join(".robo-nix")).unwrap();
        fs::write(root.join(".robo-nix").join("held.lock"), b"pid=test\n").unwrap();
        let previous = env::var_os("ROBO_NIX_LOCK_TIMEOUT");
        env::set_var("ROBO_NIX_LOCK_TIMEOUT", "0");

        let error = with_project_lock(&root, "held", || Ok(())).unwrap_err();

        assert!(error.message().contains("timed out waiting"));
        match previous {
            Some(value) => {
                env::set_var("ROBO_NIX_LOCK_TIMEOUT", value);
            }
            None => {
                env::remove_var("ROBO_NIX_LOCK_TIMEOUT");
            }
        }
        cleanup(root);
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-host-cuda-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
