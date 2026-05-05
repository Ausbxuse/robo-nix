use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::shell_env::{set_shell_env, shell_env_value};

pub(super) fn append_host_cuda_driver_bridge(envs: &mut Vec<(String, String)>) {
    let runtime = crate::runtime::read_project_runtime();
    if !runtime_needs_host_cuda_driver(&runtime) || host_cuda_auto_disabled() {
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
    let driver_dir = driver_dir.display().to_string();

    set_shell_env(envs, "ROBO_NIX_LIBCUDA_PATH", libcuda.to_string());
    set_shell_env(envs, "ROBO_NIX_HOST_LIBCUDA_AUTO", driver_dir.clone());

    if shell_env_value(envs, "TRITON_LIBCUDA_PATH").is_none()
        && env::var_os("TRITON_LIBCUDA_PATH").is_none()
    {
        set_shell_env(envs, "TRITON_LIBCUDA_PATH", driver_dir.clone());
    }

    let library_path = shell_env_value(envs, "LD_LIBRARY_PATH")
        .cloned()
        .or_else(|| env::var("LD_LIBRARY_PATH").ok())
        .unwrap_or_default();
    if !path_list_contains(&library_path, &driver_dir) {
        let value = if library_path.is_empty() {
            driver_dir
        } else {
            format!("{library_path}:{driver_dir}")
        };
        set_shell_env(envs, "LD_LIBRARY_PATH", value);
    }
}

fn runtime_needs_host_cuda_driver(runtime: &crate::runtime::ProjectRuntime) -> bool {
    runtime.cuda_wheel_version.is_some()
        || runtime
            .components
            .iter()
            .any(|component| matches!(component.as_str(), "isaac-sim"))
}

pub(super) fn append_host_graphics_bridge(envs: &mut Vec<(String, String)>) {
    let runtime = crate::runtime::read_project_runtime();
    if !runtime_needs_host_nvidia_graphics(&runtime) || host_graphics_auto_disabled() {
        return;
    }

    apply_host_graphics_bridge(
        envs,
        crate::runtime::find_host_nvidia_egl_vendor_file().as_deref(),
        crate::runtime::find_host_nvidia_vulkan_icd_file().as_deref(),
    );
}

fn apply_host_graphics_bridge(
    envs: &mut Vec<(String, String)>,
    egl_vendor_file: Option<&str>,
    vulkan_icd_file: Option<&str>,
) {
    apply_host_graphics_bridge_with_parent_env(
        envs,
        egl_vendor_file,
        vulkan_icd_file,
        env::var_os("__EGL_VENDOR_LIBRARY_FILENAMES").is_some(),
        env::var_os("VK_ICD_FILENAMES").is_some(),
    );
}

fn apply_host_graphics_bridge_with_parent_env(
    envs: &mut Vec<(String, String)>,
    egl_vendor_file: Option<&str>,
    vulkan_icd_file: Option<&str>,
    parent_egl_vendor_set: bool,
    parent_vulkan_icd_set: bool,
) {
    let mut applied = Vec::new();
    let mut applied_lib_dirs = Vec::new();
    if !parent_egl_vendor_set
        && (shell_env_value(envs, "__EGL_VENDOR_LIBRARY_FILENAMES").is_none()
            || egl_vendor_is_nix_mesa(shell_env_value(envs, "__EGL_VENDOR_LIBRARY_FILENAMES")))
        && let Some(path) = egl_vendor_file
    {
        set_shell_env(envs, "__EGL_VENDOR_LIBRARY_FILENAMES", path.to_string());
        applied.push(path.to_string());
    }
    if shell_env_value(envs, "VK_ICD_FILENAMES").is_none()
        && !parent_vulkan_icd_set
        && let Some(path) = vulkan_icd_file
    {
        set_shell_env(envs, "VK_ICD_FILENAMES", path.to_string());
        applied.push(path.to_string());
    }

    if !applied.is_empty() {
        let ldconfig = ldconfig_cache();
        for manifest in &applied {
            for dir in manifest_library_dirs(manifest, ldconfig.as_deref()) {
                if !applied_lib_dirs.iter().any(|applied| applied == &dir) {
                    applied_lib_dirs.push(dir);
                }
            }
        }
        append_library_path_dirs(envs, &applied_lib_dirs);
        set_shell_env(envs, "ROBO_NIX_HOST_GRAPHICS_AUTO", applied.join(":"));
        if !applied_lib_dirs.is_empty() {
            set_shell_env(
                envs,
                "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO",
                applied_lib_dirs.join(":"),
            );
        }
    }
}

fn runtime_needs_host_nvidia_graphics(runtime: &crate::runtime::ProjectRuntime) -> bool {
    runtime
        .components
        .iter()
        .any(|component| component == "isaac-sim")
}

fn host_cuda_auto_disabled() -> bool {
    env::var("ROBO_NIX_DISABLE_HOST_CUDA_AUTO").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn host_graphics_auto_disabled() -> bool {
    env::var("ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn egl_vendor_is_nix_mesa(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.starts_with("/nix/store/") && value.contains("mesa-"))
}

fn path_list_contains(paths: &str, needle: &str) -> bool {
    paths.split(':').any(|path| path == needle)
}

fn append_library_path_dirs(envs: &mut Vec<(String, String)>, dirs: &[String]) {
    if dirs.is_empty() {
        return;
    }

    let mut library_path = shell_env_value(envs, "LD_LIBRARY_PATH")
        .cloned()
        .or_else(|| env::var("LD_LIBRARY_PATH").ok())
        .unwrap_or_default();
    for dir in dirs {
        if path_list_contains(&library_path, dir) {
            continue;
        }
        library_path = if library_path.is_empty() {
            dir.clone()
        } else {
            format!("{library_path}:{dir}")
        };
    }
    set_shell_env(envs, "LD_LIBRARY_PATH", library_path);
}

fn manifest_library_dirs(path: &str, ldconfig: Option<&str>) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for library in manifest_library_paths(&json) {
        if let Some(dir) = resolve_manifest_library_dir(&library, ldconfig)
            && !dirs.iter().any(|existing| existing == &dir)
        {
            dirs.push(dir);
        }
    }
    dirs
}

fn manifest_library_paths(json: &serde_json::Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_manifest_library_paths(json, &mut paths);
    paths
}

fn collect_manifest_library_paths(json: &serde_json::Value, paths: &mut Vec<String>) {
    match json {
        serde_json::Value::Object(entries) => {
            for (key, value) in entries {
                if key == "library_path"
                    && let Some(path) = value.as_str()
                    && !paths.iter().any(|existing| existing == path)
                {
                    paths.push(path.to_string());
                }
                collect_manifest_library_paths(value, paths);
            }
        }
        serde_json::Value::Array(entries) => {
            for value in entries {
                collect_manifest_library_paths(value, paths);
            }
        }
        _ => {}
    }
}

fn resolve_manifest_library_dir(library: &str, ldconfig: Option<&str>) -> Option<String> {
    let path = Path::new(library);
    if path.is_absolute() {
        return path
            .is_file()
            .then(|| path.parent().map(|dir| dir.display().to_string()))
            .flatten();
    }

    ldconfig.and_then(|cache| find_ldconfig_library(cache, library))
}

fn find_ldconfig_library(cache: &str, library: &str) -> Option<String> {
    cache.lines().find_map(|line| {
        let line = line.trim();
        if line
            .strip_prefix(library)
            .is_none_or(|rest| !rest.starts_with(char::is_whitespace))
        {
            return None;
        }
        let (_, path) = line.rsplit_once(" => ")?;
        let path = Path::new(path.trim());
        path.is_file()
            .then(|| path.parent().map(|dir| dir.display().to_string()))
            .flatten()
    })
}

fn ldconfig_cache() -> Option<String> {
    let output = Command::new("ldconfig").arg("-p").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn auto_host_cuda_driver_path(envs: &[(String, String)]) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_LIBCUDA_AUTO").map(String::as_str)
}

pub(super) fn auto_host_graphics_manifests(envs: &[(String, String)]) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_GRAPHICS_AUTO").map(String::as_str)
}

pub(super) fn auto_host_graphics_library_dirs(envs: &[(String, String)]) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO").map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn host_graphics_bridge_replaces_nix_mesa_for_isaac_nvidia_runtime() {
        let temp = temp_dir("graphics-bridge-absolute");
        let lib_dir = temp.join("lib");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("libEGL_nvidia.so.0"), "").unwrap();
        let manifest = temp.join("10_nvidia.json");
        fs::write(
            &manifest,
            format!(
                r#"{{"ICD":{{"library_path":"{}"}}}}"#,
                lib_dir.join("libEGL_nvidia.so.0").display()
            ),
        )
        .unwrap();

        let mut env = vec![
            (
                "__EGL_VENDOR_LIBRARY_FILENAMES".to_string(),
                "/nix/store/abc-mesa-25.2.4/share/glvnd/egl_vendor.d/50_mesa.json".to_string(),
            ),
            ("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string()),
        ];
        let manifest = manifest.display().to_string();
        let lib_dir = lib_dir.display().to_string();
        let expected_library_path = format!("/nix/store/lib:{lib_dir}");

        apply_host_graphics_bridge(&mut env, Some(&manifest), None);

        assert_eq!(
            shell_env_value(&env, "__EGL_VENDOR_LIBRARY_FILENAMES").map(String::as_str),
            Some(manifest.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_GRAPHICS_AUTO").map(String::as_str),
            Some(manifest.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO").map(String::as_str),
            Some(lib_dir.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some(expected_library_path.as_str())
        );

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn host_graphics_bridge_keeps_user_egl_vendor() {
        let mut env = vec![(
            "__EGL_VENDOR_LIBRARY_FILENAMES".to_string(),
            "/custom/egl_vendor.json".to_string(),
        )];

        apply_host_graphics_bridge(
            &mut env,
            Some("/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json"),
            Some("/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.x86_64.json"),
        );

        assert_eq!(
            shell_env_value(&env, "__EGL_VENDOR_LIBRARY_FILENAMES").map(String::as_str),
            Some("/custom/egl_vendor.json")
        );
        assert_eq!(
            shell_env_value(&env, "VK_ICD_FILENAMES").map(String::as_str),
            Some("/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.x86_64.json")
        );
    }

    #[test]
    fn host_graphics_bridge_keeps_parent_process_graphics_choices() {
        let mut env = Vec::new();

        apply_host_graphics_bridge_with_parent_env(
            &mut env,
            Some("/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json"),
            Some("/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.x86_64.json"),
            true,
            true,
        );

        assert!(shell_env_value(&env, "__EGL_VENDOR_LIBRARY_FILENAMES").is_none());
        assert!(shell_env_value(&env, "VK_ICD_FILENAMES").is_none());
        assert!(shell_env_value(&env, "ROBO_NIX_HOST_GRAPHICS_AUTO").is_none());
    }

    #[test]
    fn manifest_library_dirs_resolves_relative_vendor_library_from_ldconfig() {
        let temp = temp_dir("graphics-bridge-relative");
        let lib_dir = temp.join("nvidia");
        fs::create_dir_all(&lib_dir).unwrap();
        let lib = lib_dir.join("libEGL_nvidia.so.0");
        fs::write(&lib, "").unwrap();
        let manifest = temp.join("10_nvidia.json");
        fs::write(
            &manifest,
            r#"{"file_format_version":"1.0.0","ICD":{"library_path":"libEGL_nvidia.so.0"}}"#,
        )
        .unwrap();
        let ldconfig = format!(
            "libEGL_nvidia.so.0 (libc6,x86-64) => {}\n",
            lib.display()
        );

        assert_eq!(
            manifest_library_dirs(&manifest.display().to_string(), Some(&ldconfig)),
            vec![lib_dir.display().to_string()]
        );

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn manifest_library_paths_collects_nested_entries_once() {
        let json = serde_json::json!({
            "ICD": {
                "library_path": "libGLX_nvidia.so.0"
            },
            "layers": [
                {
                    "library_path": "libGLX_nvidia.so.0"
                },
                {
                    "library_path": "/opt/nvidia/lib/libnvidia-egl-gbm.so.1"
                }
            ]
        });

        assert_eq!(
            manifest_library_paths(&json),
            vec![
                "libGLX_nvidia.so.0".to_string(),
                "/opt/nvidia/lib/libnvidia-egl-gbm.so.1".to_string()
            ]
        );
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("robo-nix-{name}-{nanos}"))
    }
}
