use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::shell_env::{set_shell_env, shell_env_value};

pub(in crate::command::project) fn append_host_cuda_driver_bridge(
    envs: &mut Vec<(String, String)>,
) {
    let runtime = crate::runtime::read_project_runtime();
    if !runtime_needs_host_cuda_driver(&runtime)
        || env_flag_enabled("ROBO_NIX_DISABLE_HOST_CUDA_AUTO")
    {
        return;
    }

    if shell_env_value(envs, "ROBO_NIX_LIBCUDA_PATH").is_some()
        || env::var_os("ROBO_NIX_LIBCUDA_PATH").is_some()
    {
        return;
    }

    let Some(libcuda) = crate::runtime::find_host_libcuda() else {
        return;
    };
    apply_host_cuda_driver_bridge(envs, &libcuda);
}

fn apply_host_cuda_driver_bridge(envs: &mut Vec<(String, String)>, libcuda: &str) {
    let Some(driver_dir) = Path::new(&libcuda).parent() else {
        return;
    };
    let driver_dir_path = driver_dir.to_path_buf();
    let driver_dir = driver_dir.display().to_string();

    set_shell_env(envs, "ROBO_NIX_LIBCUDA_PATH", libcuda.to_string());
    set_shell_env(envs, "ROBO_NIX_HOST_LIBCUDA_AUTO", driver_dir.clone());

    if shell_env_value(envs, "TRITON_LIBCUDA_PATH").is_none()
        && env::var_os("TRITON_LIBCUDA_PATH").is_none()
    {
        set_shell_env(envs, "TRITON_LIBCUDA_PATH", driver_dir.clone());
    }

    if driver_dir_contains_glibc(&driver_dir_path) {
        if let Some(bridge_dir) = create_libcuda_bridge(libcuda) {
            set_shell_env(envs, "ROBO_NIX_HOST_LIBCUDA_BRIDGE", bridge_dir.clone());
            append_ld_library_path(envs, &bridge_dir);
        }
        set_shell_env(
            envs,
            "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED",
            driver_dir,
        );
        return;
    }

    append_ld_library_path(envs, &driver_dir);
}

fn driver_dir_contains_glibc(driver_dir: &Path) -> bool {
    driver_dir.join("libc.so.6").exists() || driver_dir.join("ld-linux-x86-64.so.2").exists()
}

fn create_libcuda_bridge(libcuda: &str) -> Option<String> {
    let bridge_dir = PathBuf::from(".robo-nix").join("host-libs");
    fs::create_dir_all(&bridge_dir).ok()?;
    for name in ["libcuda.so.1", "libcuda.so"] {
        let link = bridge_dir.join(name);
        let _ = fs::remove_file(&link);
        replace_file_link(Path::new(libcuda), &link).ok()?;
    }
    Some(bridge_dir.display().to_string())
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
        .or_else(|| env::var("LD_LIBRARY_PATH").ok())
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

fn runtime_needs_host_cuda_driver(runtime: &crate::runtime::ProjectRuntime) -> bool {
    runtime.cuda_wheel_version.is_some()
        || runtime
            .components
            .iter()
            .any(|component| matches!(component.as_str(), "isaac-sim"))
}

pub(in crate::command::project) fn auto_host_cuda_driver_path(
    envs: &[(String, String)],
) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_LIBCUDA_AUTO").map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn host_cuda_driver_bridge_sets_minimal_runtime_vars() {
        let mut env = vec![("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string())];

        apply_host_cuda_driver_bridge(&mut env, "/run/opengl-driver/lib/libcuda.so.1");

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
    }

    #[test]
    fn host_cuda_driver_bridge_does_not_inject_glibc_dir() {
        let dir = env::temp_dir().join(format!(
            "robo-host-cuda-bridge-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("test directory should be created");
        fs::write(dir.join("libc.so.6"), b"").expect("test libc marker should be written");
        fs::write(dir.join("libcuda.so.1"), b"").expect("test libcuda marker should be written");

        let mut env = vec![("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string())];
        apply_host_cuda_driver_bridge(&mut env, &dir.join("libcuda.so.1").display().to_string());

        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some("/nix/store/lib:.robo-nix/host-libs")
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_LIBCUDA_BRIDGE").map(String::as_str),
            Some(".robo-nix/host-libs")
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_LIBCUDA_LD_LIBRARY_PATH_SKIPPED")
                .map(String::as_str),
            Some(dir.to_str().expect("test path should be utf-8"))
        );
        assert!(fs::symlink_metadata(".robo-nix/host-libs/libcuda.so.1").is_ok());
        assert!(fs::symlink_metadata(".robo-nix/host-libs/libcuda.so").is_ok());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(".robo-nix");
    }

    #[test]
    fn host_cuda_driver_bridge_does_not_duplicate_library_path() {
        let mut env = vec![
            (
                "LD_LIBRARY_PATH".to_string(),
                "/nix/store/lib:/run/opengl-driver/lib".to_string(),
            ),
            ("TRITON_LIBCUDA_PATH".to_string(), "/custom/triton".to_string()),
        ];

        apply_host_cuda_driver_bridge(&mut env, "/run/opengl-driver/lib/libcuda.so.1");

        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some("/nix/store/lib:/run/opengl-driver/lib")
        );
        assert_eq!(
            shell_env_value(&env, "TRITON_LIBCUDA_PATH").map(String::as_str),
            Some("/custom/triton")
        );
    }

    #[test]
    fn toolkit_only_runtime_does_not_require_host_cuda_driver_bridge() {
        let runtime = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: None,
            components: vec!["cuda-toolkit".to_string()],
            suggestions: Vec::new(),
        };

        assert!(!runtime_needs_host_cuda_driver(&runtime));
    }

    #[test]
    fn cuda_wheels_and_isaac_runtime_require_host_cuda_driver_bridge() {
        let cuda_wheel_runtime = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: Some("12.8".to_string()),
            components: Vec::new(),
            suggestions: Vec::new(),
        };
        let isaac_runtime = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: None,
            components: vec!["isaac-sim".to_string()],
            suggestions: Vec::new(),
        };

        assert!(runtime_needs_host_cuda_driver(&cuda_wheel_runtime));
        assert!(runtime_needs_host_cuda_driver(&isaac_runtime));
    }
}
