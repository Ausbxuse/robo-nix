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

const ROBO_MANAGED_ENV_NAMES: &[&str] = &[
    "ROBO_NIX_ACTIVE",
    "ROBO_NIX_ENV_NAME",
    "ROBO_NIX_PROMPT_PREFIX",
    "ROBO_NIX_PARENT_ZDOTDIR",
    "ROBO_NIX_RUNTIME_INPUT_KEY",
    "ROBO_NIX_RUNTIME_INPUT_FILES",
    "ROBO_NIX_COMPONENTS",
    "ROBO_NIX_MANAGED_ENV_VARS",
    "ROBO_NIX_PYTHON",
    "ROBO_NIX_HOST_GRAPHICS",
    "ROBO_NIX_LIBC_DEV",
    "ROBO_NIX_LINUX_HEADERS",
    "ROBO_NIX_LIBCUDA_PATH",
    "ROBO_NIX_HOST_LIBCUDA_AUTO",
    "ROBO_NIX_HOST_LIBCUDA_BRIDGE",
    "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED",
    "UV_PYTHON",
    "UV_PYTHON_DOWNLOADS",
    "UV_PROJECT_ENVIRONMENT",
    "UV_CACHE_DIR",
    "VIRTUAL_ENV",
    "TRITON_LIBCUDA_PATH",
    "CUDA_PATH",
    "CUDA_HOME",
    "CUDA_TOOLKIT_ROOT_DIR",
    "CUDAToolkit_ROOT",
    "CUDAHOSTCXX",
    "CC",
    "CXX",
    "CPATH",
    "C_INCLUDE_PATH",
    "LIBRARY_PATH",
    "LD_LIBRARY_PATH",
    "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
    "GBM_BACKENDS_PATH",
    "LIBGL_DRIVERS_PATH",
    "LIBVA_DRIVERS_PATH",
    "__EGL_VENDOR_LIBRARY_FILENAMES",
    "VK_ICD_FILENAMES",
    "VK_DRIVER_FILES",
    "VK_LAYER_PATH",
    "__NV_PRIME_RENDER_OFFLOAD",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__VK_LAYER_NV_optimus",
    "NVIDIA_VISIBLE_DEVICES",
    "WORKSPACE_ROOT",
];

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
        name: "ROBO_NIX_NIXGL",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description: "Override the nixGL wrapper path selected by hostGraphics.",
    },
    EnvVarSpec {
        name: "ROBO_NIX_NVIDIA_VERSION",
        visibility: EnvVarVisibility::Public,
        affects_runtime_key: true,
        description:
            "Override the detected host NVIDIA driver version used by hostGraphics = \"nixgl-nvidia\".",
    },
    EnvVarSpec {
        name: "ROBO_NIX_HOST_GRAPHICS",
        visibility: EnvVarVisibility::Internal,
        affects_runtime_key: false,
        description: "Selected host graphics wrapper policy reported by the Nix shell.",
    },
    EnvVarSpec {
        name: "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "EGL external platform config search path imported from graphics policy.",
    },
    EnvVarSpec {
        name: "GBM_BACKENDS_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "GBM backend search path imported from graphics policy.",
    },
    EnvVarSpec {
        name: "LIBGL_DRIVERS_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "OpenGL driver search path imported from host graphics wrappers.",
    },
    EnvVarSpec {
        name: "LIBVA_DRIVERS_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "VA-API driver search path imported from host graphics wrappers.",
    },
    EnvVarSpec {
        name: "__EGL_VENDOR_LIBRARY_FILENAMES",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "GLVND EGL vendor manifest path imported from graphics policy.",
    },
    EnvVarSpec {
        name: "__GLX_VENDOR_LIBRARY_NAME",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "GLVND GLX vendor name imported from graphics policy.",
    },
    EnvVarSpec {
        name: "__NV_PRIME_RENDER_OFFLOAD",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "NVIDIA PRIME render offload mode imported from the selected graphics wrapper.",
    },
    EnvVarSpec {
        name: "__VK_LAYER_NV_optimus",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "NVIDIA Vulkan layer offload mode imported from the selected graphics wrapper.",
    },
    EnvVarSpec {
        name: "VK_ICD_FILENAMES",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "Vulkan ICD manifests imported from graphics policy.",
    },
    EnvVarSpec {
        name: "VK_DRIVER_FILES",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "Vulkan driver manifests imported from graphics policy.",
    },
    EnvVarSpec {
        name: "VK_LAYER_PATH",
        visibility: EnvVarVisibility::External,
        affects_runtime_key: true,
        description: "Vulkan layer path imported from graphics policy.",
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
    ROBO_MANAGED_ENV_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
