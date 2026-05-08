use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::super::shell_env::{set_shell_env, shell_env_value};

pub(in crate::command::project) fn append_host_graphics_bridge(
    envs: &mut Vec<(String, String)>,
) {
    let runtime = crate::runtime::read_project_runtime();
    if !runtime_needs_host_nvidia_graphics(&runtime)
        || env_flag_enabled("ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO")
    {
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
    let workspace = env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    apply_host_graphics_bridge_in_workspace(
        envs,
        egl_vendor_file,
        vulkan_icd_file,
        parent_egl_vendor_set,
        parent_vulkan_icd_set,
        &workspace,
    );
}

fn apply_host_graphics_bridge_in_workspace(
    envs: &mut Vec<(String, String)>,
    egl_vendor_file: Option<&str>,
    vulkan_icd_file: Option<&str>,
    parent_egl_vendor_set: bool,
    parent_vulkan_icd_set: bool,
    workspace: &Path,
) {
    let mut applied = Vec::new();
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
        set_shell_env(envs, "ROBO_NIX_HOST_GRAPHICS_AUTO", applied.join(":"));
        if let Some(bridge_dir) =
            materialize_host_graphics_bridge(workspace, &applied, ldconfig.as_deref())
        {
            let bridge_dir = bridge_dir.display().to_string();
            append_library_path_dirs(envs, &[bridge_dir.clone()]);
            set_shell_env(envs, "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO", bridge_dir);
        }
    }
}

fn runtime_needs_host_nvidia_graphics(runtime: &crate::runtime::ProjectRuntime) -> bool {
    runtime
        .components
        .iter()
        .any(|component| component == "host-nvidia-gl")
}

fn egl_vendor_is_nix_mesa(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.starts_with("/nix/store/") && value.contains("mesa-"))
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
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

fn path_list_contains(paths: &str, needle: &str) -> bool {
    paths.split(':').any(|path| path == needle)
}

fn materialize_host_graphics_bridge(
    workspace: &Path,
    manifests: &[String],
    ldconfig: Option<&str>,
) -> Option<std::path::PathBuf> {
    let mut libraries = BTreeSet::new();
    for manifest in manifests {
        for library in manifest_library_paths(manifest, ldconfig) {
            libraries.insert(library);
        }
    }
    if let Some(cache) = ldconfig {
        if let Some(glx) = find_ldconfig_library_path(cache, "libGLX_nvidia.so.0") {
            if let Some(driver_version) = nvidia_driver_version(&glx) {
                for library in find_versioned_host_graphics_libraries(cache, &driver_version) {
                    libraries.insert(library);
                }
            }
            libraries.insert(glx);
        }
    }
    if libraries.is_empty() {
        return None;
    }

    let bridge_dir = workspace.join(".robo-nix").join("host-graphics").join("lib");
    fs::create_dir_all(&bridge_dir).ok()?;
    for library in libraries {
        link_host_graphics_library(&bridge_dir, &library).ok()?;
        for dependency in host_graphics_library_dependencies(&library) {
            let _ = link_host_graphics_library(&bridge_dir, &dependency);
        }
    }

    Some(bridge_dir)
}

fn manifest_library_paths(path: &str, ldconfig: Option<&str>) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };

    let mut libraries = Vec::new();
    for library in manifest_library_names(&json) {
        if let Some(path) = resolve_manifest_library_path(&library, ldconfig)
            && !libraries.iter().any(|existing| existing == &path)
        {
            libraries.push(path);
        }
    }
    libraries
}

fn manifest_library_names(json: &serde_json::Value) -> Vec<String> {
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

fn resolve_manifest_library_path(library: &str, ldconfig: Option<&str>) -> Option<String> {
    let path = Path::new(library);
    if path.is_absolute() {
        return path.is_file().then(|| path.display().to_string());
    }

    ldconfig.and_then(|cache| find_ldconfig_library_path(cache, library))
}

fn find_ldconfig_library_path(cache: &str, library: &str) -> Option<String> {
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
        path.is_file().then(|| path.display().to_string())
    })
}

fn host_graphics_library_dependencies(library: &str) -> Vec<String> {
    let output = Command::new("ldd").arg(library).output().ok();
    let Some(output) = output.filter(|output| output.status.success()) else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.rsplit_once(" => ").map(|(_, path)| path.trim()))
        .filter_map(|path| path.split_whitespace().next())
        .filter(|path| host_graphics_library_name(Path::new(path)))
        .map(str::to_string)
        .collect()
}

fn nvidia_driver_version(library: &str) -> Option<String> {
    let path = fs::canonicalize(library).ok()?;
    let name = path.file_name()?.to_str()?;
    for prefix in ["libGLX_nvidia.so.", "libEGL_nvidia.so."] {
        if let Some(version) = name.strip_prefix(prefix)
            && version != "0"
        {
            return Some(version.to_string());
        }
    }
    None
}

fn find_versioned_host_graphics_libraries(cache: &str, driver_version: &str) -> Vec<String> {
    cache
        .lines()
        .filter_map(|line| line.rsplit_once(" => ").map(|(_, path)| path.trim()))
        .filter_map(|path| path.split_whitespace().next())
        .filter(|path| {
            let canonical = fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
            canonical
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.contains(driver_version) && host_graphics_library_name(Path::new(name))
                })
        })
        .map(str::to_string)
        .collect()
}

fn host_graphics_library_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.contains("nvidia")
                || name.starts_with("libGLX_")
                || name.starts_with("libEGL_")
                || name.starts_with("libGLES")
        })
}

#[cfg(unix)]
fn link_host_graphics_library(bridge_dir: &Path, library: &str) -> std::io::Result<()> {
    let library = Path::new(library);
    let Some(name) = library.file_name() else {
        return Ok(());
    };
    let link = bridge_dir.join(name);
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(library, link)
}

#[cfg(not(unix))]
fn link_host_graphics_library(_bridge_dir: &Path, _library: &str) -> std::io::Result<()> {
    Ok(())
}

fn ldconfig_cache() -> Option<String> {
    let output = Command::new("ldconfig").arg("-p").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

pub(in crate::command::project) fn auto_host_graphics_manifests(
    envs: &[(String, String)],
) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_GRAPHICS_AUTO").map(String::as_str)
}

pub(in crate::command::project) fn auto_host_graphics_library_dirs(
    envs: &[(String, String)],
) -> Option<&str> {
    shell_env_value(envs, "ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO").map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn host_nvidia_gl_runtime_requires_host_nvidia_graphics_bridge() {
        let host_nvidia_gl_runtime = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: None,
            components: vec!["host-nvidia-gl".to_string()],
            suggestions: Vec::new(),
        };
        let isaac_runtime_without_host_bridge = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: None,
            components: vec!["isaac-sim".to_string()],
            suggestions: Vec::new(),
        };
        let base_runtime = crate::runtime::ProjectRuntime {
            schema_version: None,
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: None,
            components: vec!["base".to_string()],
            suggestions: Vec::new(),
        };

        assert!(runtime_needs_host_nvidia_graphics(&host_nvidia_gl_runtime));
        assert!(!runtime_needs_host_nvidia_graphics(
            &isaac_runtime_without_host_bridge
        ));
        assert!(!runtime_needs_host_nvidia_graphics(&base_runtime));
    }

    #[test]
    fn host_graphics_bridge_replaces_nix_mesa_for_isaac_nvidia_runtime() {
        let workspace = temp_dir("graphics-bridge-workspace");
        let temp = temp_dir("graphics-bridge-absolute");
        let lib_dir = temp.join("lib");
        let bridge_dir = workspace.join(".robo-nix").join("host-graphics").join("lib");
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
        let bridge_dir_value = bridge_dir.display().to_string();
        let expected_library_path = format!("/nix/store/lib:{bridge_dir_value}");

        apply_host_graphics_bridge_in_workspace(
            &mut env,
            Some(&manifest),
            None,
            false,
            false,
            &workspace,
        );

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
            Some(bridge_dir_value.as_str())
        );
        assert_eq!(
            shell_env_value(&env, "LD_LIBRARY_PATH").map(String::as_str),
            Some(expected_library_path.as_str())
        );
        assert!(bridge_dir.join("libEGL_nvidia.so.0").is_symlink());

        let _ = fs::remove_dir_all(temp);
        let _ = fs::remove_dir_all(workspace);
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
    fn host_graphics_bridge_materializes_relative_vendor_library_from_ldconfig() {
        let workspace = temp_dir("graphics-bridge-workspace");
        let temp = temp_dir("graphics-bridge-relative");
        let lib_dir = temp.join("nvidia");
        let bridge_dir = workspace.join(".robo-nix").join("host-graphics").join("lib");
        fs::create_dir_all(&lib_dir).unwrap();
        let lib = lib_dir.join("libEGL_nvidia.so.0");
        let glx = lib_dir.join("libGLX_nvidia.so.0");
        fs::write(&lib, "").unwrap();
        fs::write(&glx, "").unwrap();
        let manifest = temp.join("10_nvidia.json");
        fs::write(
            &manifest,
            r#"{"file_format_version":"1.0.0","ICD":{"library_path":"libEGL_nvidia.so.0"}}"#,
        )
        .unwrap();
        let ldconfig = format!(
            "libEGL_nvidia.so.0 (libc6,x86-64) => {}\nlibGLX_nvidia.so.0 (libc6,x86-64) => {}\n",
            lib.display(),
            glx.display()
        );

        let bridge = materialize_host_graphics_bridge(
            &workspace,
            &[manifest.display().to_string()],
            Some(&ldconfig),
        )
        .expect("bridge should be materialized");

        assert_eq!(bridge, bridge_dir);
        assert!(bridge_dir.join("libEGL_nvidia.so.0").is_symlink());
        assert!(bridge_dir.join("libGLX_nvidia.so.0").is_symlink());

        let _ = fs::remove_dir_all(temp);
        let _ = fs::remove_dir_all(workspace);
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
            manifest_library_names(&json),
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
