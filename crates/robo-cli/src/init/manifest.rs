use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;

use super::spec::ProjectSpec;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manifest {
    pub(super) components: BTreeMap<String, Component>,
    pub(super) profiles: BTreeMap<String, Profile>,
    pub(super) runtime_inference: RuntimeInference,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Component {
    pub(super) category: String,
    pub(super) description: String,
    #[serde(default)]
    pub(super) provides: Vec<String>,
    pub(super) scaffold_directories: Vec<String>,
    pub(super) supported_systems: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Profile {
    pub(super) description: String,
    pub(super) components: Vec<String>,
    pub(super) python_version: String,
    pub(super) supported_systems: Vec<String>,
    pub(super) workspace_root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeInference {
    pub(super) default_profile: String,
    pub(super) dependency_rules: Vec<DependencyRule>,
    pub(super) compound_dependency_rules: Vec<CompoundDependencyRule>,
    pub(super) workspace_directory_rules: Vec<WorkspaceDirectoryRule>,
    pub(super) script_discovery: ScriptDiscovery,
    pub(super) script_rules: Vec<ScriptRule>,
    pub(super) cuda_marker_scan: CudaMarkerScan,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DependencyRule {
    pub(super) dependencies: Vec<String>,
    #[serde(default)]
    pub(super) requires: Vec<String>,
    #[serde(default)]
    pub(super) components: Vec<String>,
    pub(super) note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompoundDependencyRule {
    pub(super) dependencies_all: Vec<Vec<String>>,
    #[serde(default)]
    pub(super) requires: Vec<String>,
    #[serde(default)]
    pub(super) components: Vec<String>,
    pub(super) note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceDirectoryRule {
    pub(super) root: String,
    pub(super) name_contains: Vec<String>,
    #[serde(default)]
    pub(super) requires: Vec<String>,
    #[serde(default)]
    pub(super) components: Vec<String>,
    pub(super) note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScriptDiscovery {
    pub(super) roots: Vec<String>,
    pub(super) names: Vec<String>,
    pub(super) prefixes: Vec<String>,
    pub(super) daemon_text_contains: Vec<String>,
    pub(super) checkout_function: String,
    pub(super) path_root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScriptRule {
    pub(super) text_contains: Vec<String>,
    #[serde(default)]
    pub(super) requires: Vec<String>,
    #[serde(default)]
    pub(super) components: Vec<String>,
    pub(super) note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CudaMarkerScan {
    pub(super) max_depth: usize,
    pub(super) max_files: usize,
    pub(super) source_extensions: Vec<String>,
    pub(super) build_files: Vec<String>,
    pub(super) text_contains: Vec<String>,
    pub(super) skip_names: Vec<String>,
    #[serde(default)]
    pub(super) requires: Vec<String>,
    pub(super) component: String,
    pub(super) note: String,
}

pub(super) fn load_manifest() -> Result<Manifest, String> {
    let path = env::var("ROBO_NIX_COMPONENT_MANIFEST")
        .map_err(|_| "ROBO_NIX_COMPONENT_MANIFEST is not set.".to_string())?;
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read component manifest {path}: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("failed to parse component manifest: {err}"))
}

pub(super) fn list_profiles(manifest: &Manifest) {
    for (name, profile) in &manifest.profiles {
        println!(
            "{:<18} {:<30} {}",
            name,
            profile.supported_systems.join(","),
            profile.description
        );
    }
}

pub(super) fn list_components(manifest: &Manifest) {
    for (name, component) in &manifest.components {
        println!(
            "{:<18} {:<12} {:<30} {}",
            name,
            component.category,
            component.supported_systems.join(","),
            component.description
        );
    }
}

pub(super) fn profile_names(manifest: &Manifest) -> Vec<String> {
    let mut names = manifest.profiles.keys().cloned().collect::<Vec<_>>();
    if let Some(index) = names
        .iter()
        .position(|name| name == &manifest.runtime_inference.default_profile)
    {
        let default_profile = names.remove(index);
        names.insert(0, default_profile);
    }
    names
}

pub(super) fn resolve_profile_selection(profiles: &[String], selection: &str) -> Option<String> {
    if let Ok(index) = selection.parse::<usize>() {
        return profiles.get(index.checked_sub(1)?).cloned();
    }
    profiles
        .iter()
        .find(|profile| *profile == selection)
        .cloned()
}

pub(super) fn validate(manifest: &Manifest, spec: &ProjectSpec) -> Result<(), String> {
    if spec.supported_systems.is_empty() {
        return Err("expected at least one value for systems".to_string());
    }
    for component in &spec.components {
        if !manifest.components.contains_key(component) {
            return Err(format!("unknown component: {component}"));
        }
    }
    for requirement in &spec.requirements {
        validate_requirements(manifest, std::slice::from_ref(&requirement.id))?;
    }
    for rule in &manifest.runtime_inference.dependency_rules {
        validate_requirements(manifest, &rule.requires)?;
        for component in &rule.components {
            if !manifest.components.contains_key(component) {
                return Err(format!(
                    "runtime inference rule references unknown component: {component}"
                ));
            }
        }
    }
    for rule in &manifest.runtime_inference.compound_dependency_rules {
        validate_requirements(manifest, &rule.requires)?;
        for component in &rule.components {
            if !manifest.components.contains_key(component) {
                return Err(format!(
                    "compound runtime inference rule references unknown component: {component}"
                ));
            }
        }
    }
    for rule in &manifest.runtime_inference.workspace_directory_rules {
        validate_requirements(manifest, &rule.requires)?;
        for component in &rule.components {
            if !manifest.components.contains_key(component) {
                return Err(format!(
                    "workspace runtime inference rule references unknown component: {component}"
                ));
            }
        }
    }
    for rule in &manifest.runtime_inference.script_rules {
        validate_requirements(manifest, &rule.requires)?;
        for component in &rule.components {
            if !manifest.components.contains_key(component) {
                return Err(format!(
                    "script runtime inference rule references unknown component: {component}"
                ));
            }
        }
    }
    validate_requirements(manifest, &manifest.runtime_inference.cuda_marker_scan.requires)?;
    if !manifest
        .components
        .contains_key(&manifest.runtime_inference.cuda_marker_scan.component)
    {
        return Err(format!(
            "CUDA marker scan references unknown component: {}",
            manifest.runtime_inference.cuda_marker_scan.component
        ));
    }
    Ok(())
}

fn validate_requirements(manifest: &Manifest, requirements: &[String]) -> Result<(), String> {
    for requirement in requirements {
        if requirement.starts_with("host.") {
            continue;
        }
        if !manifest
            .components
            .values()
            .any(|component| component.provides.iter().any(|item| item == requirement))
        {
            return Err(format!(
                "runtime inference rule references unsatisfied requirement: {requirement}"
            ));
        }
    }
    Ok(())
}
