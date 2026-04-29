use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::quoted_value;

pub(crate) struct ProjectRuntime {
    pub(crate) schema_version: Option<String>,
    pub(crate) env_name: String,
    pub(crate) python_version: String,
    pub(crate) cuda_wheel_version: Option<String>,
    pub(crate) components: Vec<String>,
    pub(crate) suggestions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeWhy {
    pub(crate) env_name: String,
    pub(crate) python_version: String,
    pub(crate) profile: Option<String>,
    pub(crate) components: Vec<WhyEntry>,
    pub(crate) required_directories: Vec<WhyEntry>,
    pub(crate) required_files: Vec<WhyEntry>,
    pub(crate) bootstrap_scripts: Vec<WhyEntry>,
    pub(crate) suggestions: Vec<WhyEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WhyEntry {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) reason: String,
    pub(crate) remove_hint: String,
    pub(crate) remediation_hint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    profiles: BTreeMap<String, RuntimeProfile>,
    #[serde(default)]
    runtime_inference: RuntimeInference,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProfile {
    components: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInference {
    #[serde(default)]
    dependency_rules: Vec<DependencyRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyRule {
    dependencies: Vec<String>,
    components: Vec<String>,
    note: String,
}

pub(crate) struct ExpectedComponent {
    pub(crate) name: String,
    pub(crate) reason: String,
}

struct ProjectProvenance {
    profile: Option<String>,
    inferred: Vec<String>,
    component_reasons: HashMap<String, ComponentReason>,
    required_dirs: Vec<String>,
    required_files: Vec<String>,
    bootstrap_scripts: Vec<String>,
}

#[derive(Clone)]
struct ComponentReason {
    source: String,
    reason: String,
}

pub(crate) fn read_project_runtime() -> ProjectRuntime {
    let robo_nix = fs::read_to_string("robo.nix").unwrap_or_default();
    ProjectRuntime {
        schema_version: nix_unquoted_attr_value(&robo_nix, "schemaVersion").map(ToOwned::to_owned),
        env_name: nix_attr_value(&robo_nix, "envName")
            .unwrap_or("project")
            .to_string(),
        python_version: nix_attr_value(&robo_nix, "pythonVersion")
            .unwrap_or("unknown")
            .to_string(),
        cuda_wheel_version: nix_attr_value(&robo_nix, "cudaWheelVersion").map(ToOwned::to_owned),
        components: nix_list_values(&robo_nix, "components"),
        suggestions: nix_attr_values(&robo_nix, "path"),
    }
}

pub(crate) fn infer_cuda_wheel_version_from_uv_lock() -> Option<String> {
    let lock = fs::read_to_string("uv.lock").ok()?;
    infer_cuda_wheel_version_from_uv_lock_text(&lock)
}

pub(crate) fn infer_cuda_wheel_version_from_uv_lock_text(text: &str) -> Option<String> {
    let mut package_name: Option<String> = None;
    let mut best: Option<(u32, u32)> = None;

    for line in text.lines() {
        let line = line.trim();

        if line.starts_with("[[package]]") {
            package_name = None;
            continue;
        }

        if let Some(value) = line.strip_prefix("name = ") {
            if is_cuda_package_name(&extract_quoted(value).unwrap_or("")) {
                package_name = extract_quoted(value).map(ToOwned::to_owned);
            } else {
                package_name = None;
            }
            continue;
        }

        if line.starts_with("version = ")
            && let Some(name) = package_name.as_deref()
            && is_cuda_package_name(name)
        {
            if let Some(raw) = extract_quoted(line) {
                if let Some(version) = parse_major_minor(raw) {
                    if best.is_none_or(|current| version > current) {
                        best = Some(version);
                    }
                }
            }
        }
    }

    best.map(|(major, minor)| format!("{major}.{minor}"))
}

pub(crate) fn cuda_root_from_env() -> Option<String> {
    env::var("ROBO_NIX_CUDA_ROOT")
        .ok()
        .filter(|root| Path::new(root).is_dir())
        .or_else(|| env::var("CUDA_HOME").ok().filter(|root| Path::new(root).is_dir()))
        .or_else(|| env::var("CUDA_PATH").ok().filter(|root| Path::new(root).is_dir()))
        .or_else(|| {
            let fallback = Path::new("/usr/local/cuda");
            if fallback.exists() {
                Some(fallback.to_string_lossy().to_string())
            } else {
                None
            }
        })
}

pub(crate) fn cuda_version_from_root() -> Option<String> {
    let root = cuda_root_from_env()?;
    let root = Path::new(&root);
    read_cuda_version_file(root)
        .or_else(|| parse_cuda_version_from_path(root))
        .or_else(|| read_cuda_version_with_nvcc(root))
}

fn read_cuda_version_file(root: &Path) -> Option<String> {
    let path = root.join("version.txt");
    if !path.is_file() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(cuda_major_minor_version)
}

fn parse_cuda_version_from_path(root: &Path) -> Option<String> {
    let file_name = root.file_name()?.to_str()?;
    file_name.split('-').find_map(cuda_major_minor_version)
}

fn read_cuda_version_with_nvcc(root: &Path) -> Option<String> {
    let nvcc = root.join("bin").join("nvcc");
    if !nvcc.is_file() {
        return None;
    }
    let output = Command::new(nvcc).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let text2 = String::from_utf8_lossy(&output.stderr);
    find_cuda_release_version(&text).or_else(|| find_cuda_release_version(&text2))
}

fn find_cuda_release_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(index) = line.find("release ") {
            let rest = &line[index + "release ".len()..];
            if let Some(version) = cuda_major_minor_version(rest) {
                return Some(version);
            }
        }
    }
    None
}

pub(crate) fn cuda_release_version_from_text(text: &str) -> Option<String> {
    find_cuda_release_version(text)
}

fn cuda_major_minor_version(text: &str) -> Option<String> {
    parse_major_minor(text).map(|(major, minor)| format!("{major}.{minor}"))
}

fn parse_major_minor(text: &str) -> Option<(u32, u32)> {
    let mut parts = text
        .split(|ch: char| ch == '.' || ch == ' ' || ch == '_' || ch == '-')
        .filter(|part| !part.is_empty())
        .map(|part| part.trim_matches(|c: char| !c.is_ascii_digit() && c != '.'));

    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

pub(crate) fn build_runtime_why(runtime: &ProjectRuntime) -> RuntimeWhy {
    let robo_nix = fs::read_to_string("robo.nix").unwrap_or_default();
    let provenance = ProjectProvenance {
        profile: nix_attr_value(&robo_nix, "profile").map(ToOwned::to_owned),
        inferred: nix_list_values(&robo_nix, "inferred"),
        component_reasons: component_reasons(&robo_nix),
        required_dirs: nix_list_values(&robo_nix, "requiredDirectories"),
        required_files: nix_list_values(&robo_nix, "requiredFiles"),
        bootstrap_scripts: nix_source_scripts(&robo_nix),
    };
    let profile_components = provenance
        .profile
        .as_deref()
        .and_then(profile_components)
        .unwrap_or_default();

    RuntimeWhy {
        env_name: runtime.env_name.clone(),
        python_version: runtime.python_version.clone(),
        profile: provenance.profile.clone(),
        components: runtime
            .components
            .iter()
            .map(|component| explain_component(component, &provenance, &profile_components))
            .collect(),
        required_directories: provenance
            .required_dirs
            .iter()
            .map(|path| explain_required_path("directory", path, &provenance))
            .collect(),
        required_files: provenance
            .required_files
            .iter()
            .map(|path| explain_required_path("file", path, &provenance))
            .collect(),
        bootstrap_scripts: provenance
            .bootstrap_scripts
            .iter()
            .map(|path| WhyEntry {
                name: path.clone(),
                source: if provenance.inferred.is_empty() {
                    "manual config".to_string()
                } else {
                    "workspace inference".to_string()
                },
                reason: first_inference(&provenance)
                    .unwrap_or_else(|| "listed in the bootstrap block in robo.nix".to_string()),
                remove_hint: format!("remove `{path}` from the bootstrap block in robo.nix"),
                remediation_hint: format!(
                    "create `{path}` or remove it if this project does not need that bootstrap step"
                ),
            })
            .collect(),
        suggestions: runtime
            .suggestions
            .iter()
            .map(|path| WhyEntry {
                name: path.clone(),
                source: "workspace inference".to_string(),
                reason: "optional low-confidence source/runtime inference".to_string(),
                remove_hint: "delete this entry from provenance.suggestions in robo.nix".to_string(),
                remediation_hint: format!(
                    "promote `{path}` to requiredFiles or requiredDirectories only if bootstrap truly depends on it"
                ),
            })
            .collect(),
    }
}

pub(crate) fn expected_components_from_pyproject(text: &str) -> Vec<ExpectedComponent> {
    let Some(manifest) = read_runtime_manifest() else {
        return Vec::new();
    };
    let lower = text.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut expected = Vec::new();

    for rule in manifest.runtime_inference.dependency_rules {
        if !rule
            .dependencies
            .iter()
            .any(|name| dependency_is_listed(&lower, name))
        {
            continue;
        }
        for component in rule.components {
            if seen.insert(component.clone()) {
                expected.push(ExpectedComponent {
                    name: component,
                    reason: rule.note.clone(),
                });
            }
        }
    }

    expected
}

fn explain_component(
    component: &str,
    provenance: &ProjectProvenance,
    profile_components: &[String],
) -> WhyEntry {
    if let Some(profile) = provenance
        .profile
        .as_deref()
        .filter(|_| profile_components.iter().any(|item| item == component))
    {
        WhyEntry {
            name: component.to_string(),
            source: "profile".to_string(),
            reason: format!("selected by the `{profile}` profile"),
            remove_hint: "choose a different profile with `robo init --profile ... --force`, or edit `components` in robo.nix".to_string(),
            remediation_hint: "keep profile components unless the project has a known smaller runtime contract".to_string(),
        }
    } else if let Some(reason) = provenance.component_reasons.get(component) {
        WhyEntry {
            name: component.to_string(),
            source: reason.source.clone(),
            reason: reason.reason.clone(),
            remove_hint: format!(
                "remove `{component}` from `components` in robo.nix if the inference is wrong"
            ),
            remediation_hint: "run `robo check --why` after edits to confirm the runtime contract still matches the project".to_string(),
        }
    } else if !provenance.inferred.is_empty() {
        WhyEntry {
            name: component.to_string(),
            source: "inference".to_string(),
            reason: first_inference(provenance)
                .unwrap_or_else(|| "inferred from pyproject.toml or workspace probes".to_string()),
            remove_hint: format!(
                "remove `{component}` from `components` in robo.nix if the inference is wrong"
            ),
            remediation_hint: "run `robo check --why` after edits to confirm the inferred runtime still matches the project".to_string(),
        }
    } else {
        WhyEntry {
            name: component.to_string(),
            source: "manual config".to_string(),
            reason: "listed directly in robo.nix".to_string(),
            remove_hint: format!("remove `{component}` from `components` in robo.nix"),
            remediation_hint: "keep manual components that provide native libraries, simulators, GPU, graphics, ROS, or compiler tooling this project needs".to_string(),
        }
    }
}

fn explain_required_path(kind: &str, path: &str, provenance: &ProjectProvenance) -> WhyEntry {
    let (source, reason) = if provenance.inferred.is_empty() {
        (
            "manual config".to_string(),
            format!("listed in required {kind}s in robo.nix"),
        )
    } else if path.starts_with("third_party/") {
        (
            "workspace scan".to_string(),
            "third_party checkout detected during init".to_string(),
        )
    } else {
        (
            "workspace inference".to_string(),
            format!("listed in required {kind}s in robo.nix"),
        )
    };

    WhyEntry {
        name: path.to_string(),
        source,
        reason,
        remove_hint: format!(
            "remove `{path}` from required{} in robo.nix",
            if kind == "file" {
                "Files"
            } else {
                "Directories"
            }
        ),
        remediation_hint: format!(
            "create `{path}` or remove it if the project does not require this {kind}"
        ),
    }
}

fn first_inference(provenance: &ProjectProvenance) -> Option<String> {
    provenance.inferred.first().cloned()
}

fn profile_components(profile: &str) -> Option<Vec<String>> {
    read_runtime_manifest().and_then(|manifest| {
        manifest
            .profiles
            .get(profile)
            .map(|profile| profile.components.clone())
    })
}

fn read_runtime_manifest() -> Option<RuntimeManifest> {
    let manifest_path = env::var("ROBO_NIX_COMPONENT_MANIFEST").ok()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    serde_json::from_str(&manifest).ok()
}

fn dependency_is_listed(pyproject_lower: &str, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    pyproject_lower.contains(&format!("\"{name}"))
        || pyproject_lower.contains(&format!("'{name}"))
}

fn component_reasons(text: &str) -> HashMap<String, ComponentReason> {
    let mut reasons = HashMap::new();
    for item in nix_attr_set_values(text, "componentReasons") {
        let Some(name) = item.get("name") else {
            continue;
        };
        let source = item
            .get("source")
            .cloned()
            .unwrap_or_else(|| "inference".to_string());
        let reason = item
            .get("reason")
            .cloned()
            .unwrap_or_else(|| "listed in provenance.componentReasons".to_string());
        reasons.insert(name.clone(), ComponentReason { source, reason });
    }
    reasons
}

fn nix_attr_value<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix(name) else {
            continue;
        };
        return quoted_value(value);
    }
    None
}

fn nix_unquoted_attr_value<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix(name) else {
            continue;
        };
        let (_, value) = value.split_once('=')?;
        return Some(value.trim().trim_end_matches(';').trim());
    }
    None
}

fn nix_attr_set_values(text: &str, name: &str) -> Vec<BTreeMap<String, String>> {
    let mut in_list = false;
    let mut in_item = false;
    let mut current = BTreeMap::new();
    let mut values = Vec::new();

    for line in text.lines().map(str::trim) {
        if !in_list {
            in_list = line.starts_with(name) && line.contains('[');
            continue;
        }
        if line.starts_with(']') {
            break;
        }
        if line.starts_with('{') {
            in_item = true;
            current.clear();
            continue;
        }
        if line.starts_with('}') {
            in_item = false;
            values.push(current.clone());
            continue;
        }
        if in_item {
            if let Some((key, _)) = line.split_once('=') {
                if let Some(value) = quoted_value(line) {
                    current.insert(key.trim().to_string(), value.to_string());
                }
            }
        }
    }

    values
}

fn nix_list_values(text: &str, name: &str) -> Vec<String> {
    let mut in_list = false;
    let mut values = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !in_list {
            in_list = line.starts_with(name) && line.contains('[');
            continue;
        }
        if line.starts_with(']') {
            break;
        }
        if let Some(value) = quoted_item(line) {
            values.push(value.to_string());
        }
    }
    values
}

fn nix_attr_values(text: &str, name: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix(name).and_then(quoted_value)
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn nix_source_scripts(text: &str) -> Vec<String> {
    let mut scripts = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(start) = line.find("$WORKSPACE_ROOT/") else {
            continue;
        };
        let value = &line[start + "$WORKSPACE_ROOT/".len()..];
        let end = value.find('"').unwrap_or(value.len());
        let script = &value[..end];
        if !script.is_empty() && !scripts.iter().any(|item| item == script) {
            scripts.push(script.to_string());
        }
    }
    scripts
}

fn quoted_item(text: &str) -> Option<&str> {
    let value = text.trim().trim_end_matches(';').trim_end_matches(',');
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &value[quote.len_utf8()..];
    let end = body.find(quote)?;
    Some(&body[..end])
}

fn extract_quoted(text: &str) -> Option<&str> {
    let start = text.find('"')?;
    let text = &text[start + 1..];
    let end = text.find('"')?;
    Some(&text[..end])
}

fn is_cuda_package_name(name: &str) -> bool {
    name.contains("-cu")
        && name
            .rsplit("-cu")
            .next()
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_cuda_wheel_version_from_uv_lock() {
        let lock = r#"
[[package]]
name = "numpydantic"
version = "1.6.7"

[[package]]
name = "nvidia-cudnn-cu12"
version = "9.5.1.17"

[[package]]
name = "nvidia-cuda-runtime-cu12"
version = "12.6.77"
"#;
        assert_eq!(
            infer_cuda_wheel_version_from_uv_lock_text(lock),
            Some("12.6".to_string())
        );
    }

    #[test]
    fn infers_max_cuda_minor_from_uv_lock() {
        let lock = r#"
[[package]]
name = "nvidia-cublas-cu12"
version = "12.5.3"

[[package]]
name = "nvidia-cuda-runtime-cu12"
version = "12.6.77"
"#;
        assert_eq!(
            infer_cuda_wheel_version_from_uv_lock_text(lock),
            Some("12.6".to_string())
        );
    }

    #[test]
    fn parses_cuda_version_from_path() {
        assert_eq!(parse_cuda_version_from_path(std::path::Path::new("/nix/store/x-robo-cuda-toolkit-12.8")), Some("12.8".to_string()));
        assert_eq!(parse_cuda_version_from_path(std::path::Path::new("/usr/local/cuda-12.3")), Some("12.3".to_string()));
    }

    #[test]
    fn parses_release_line() {
        let output = "Cuda compilation tools, release 12.8, V12.8.0";
        assert_eq!(find_cuda_release_version(output), Some("12.8".to_string()));
    }
}
