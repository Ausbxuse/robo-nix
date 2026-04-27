use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;

use crate::quoted_value;

pub(crate) struct ProjectRuntime {
    pub(crate) schema_version: Option<String>,
    pub(crate) env_name: String,
    pub(crate) python_version: String,
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProfile {
    components: Vec<String>,
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
        components: nix_list_values(&robo_nix, "components"),
        suggestions: nix_attr_values(&robo_nix, "path"),
    }
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
                reason: "optional low-confidence vendor/runtime inference".to_string(),
                remove_hint: "delete this entry from provenance.suggestions in robo.nix".to_string(),
                remediation_hint: format!(
                    "promote `{path}` to requiredFiles or requiredDirectories only if bootstrap truly depends on it"
                ),
            })
            .collect(),
    }
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
            remediation_hint: "run `robo doctor --why` after edits to confirm the runtime contract still matches the project".to_string(),
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
            remediation_hint: "run `robo doctor --why` after edits to confirm the inferred runtime still matches the project".to_string(),
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
    WhyEntry {
        name: path.to_string(),
        source: if provenance.inferred.is_empty() {
            "manual config".to_string()
        } else {
            "workspace inference".to_string()
        },
        reason: first_inference(provenance)
            .unwrap_or_else(|| format!("listed in required {kind}s in robo.nix")),
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
    let manifest_path = env::var("ROBO_NIX_COMPONENT_MANIFEST").ok()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let manifest: RuntimeManifest = serde_json::from_str(&manifest).ok()?;
    manifest
        .profiles
        .get(profile)
        .map(|profile| profile.components.clone())
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
