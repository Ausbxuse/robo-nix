use std::env;
use std::path::PathBuf;

use super::InitArgs;
use super::manifest::{Manifest, validate};
use super::probe::probe_project;
use super::render::{render_flake, render_project};
use super::spec::{ComponentProvenance, ProjectSpec, dedupe_all, parse_list, push_unique};

// Keep init as a flat pipeline. Add new inference coverage in metadata/probe code,
// then apply it here; do not turn this into a builder framework.
pub(super) struct ProjectDraft {
    pub(super) target_dir: PathBuf,
    pub(super) spec: ProjectSpec,
}

pub(super) struct ProjectPlan {
    pub(super) target_dir: PathBuf,
    pub(super) source_url: String,
    pub(super) flake: String,
    pub(super) project: String,
    pub(super) spec: ProjectSpec,
}

pub(super) fn build_draft(args: &InitArgs, manifest: &Manifest) -> Result<ProjectDraft, String> {
    let profile = args
        .profile
        .as_deref()
        .unwrap_or(&manifest.runtime_inference.default_profile);
    let target_dir = args.target.clone().unwrap_or_else(|| PathBuf::from("."));
    let mut spec = ProjectSpec::from_profile(profile, manifest)?;

    apply_target_defaults(&target_dir, &mut spec);
    if !args.no_probe {
        spec.apply_probe(probe_project(&target_dir, manifest));
    }
    apply_cli_overrides(args, &mut spec);
    dedupe_all(&mut spec);

    Ok(ProjectDraft { target_dir, spec })
}

pub(super) fn finish_plan(
    args: &InitArgs,
    manifest: &Manifest,
    mut draft: ProjectDraft,
) -> Result<ProjectPlan, String> {
    dedupe_all(&mut draft.spec);
    validate(manifest, &draft.spec)?;

    let source_url = args
        .robo_nix_url
        .clone()
        .or_else(|| env::var("ROBO_NIX_DEFAULT_SOURCE_URL").ok())
        .unwrap_or_else(|| "github:ausbxuse/robo-nix".to_string());
    let flake = render_flake(&source_url);
    let project = render_project(&draft.spec);

    Ok(ProjectPlan {
        target_dir: draft.target_dir,
        source_url,
        flake,
        project,
        spec: draft.spec,
    })
}

fn apply_target_defaults(target_dir: &PathBuf, spec: &mut ProjectSpec) {
    if spec.env_name != "project" {
        return;
    }
    if let Some(name) = target_dir.file_name().and_then(|name| name.to_str()) {
        if !name.is_empty() && name != "." {
            spec.env_name = name.to_string();
        }
    }
}

fn apply_cli_overrides(args: &InitArgs, spec: &mut ProjectSpec) {
    if let Some(components) = &args.components {
        spec.components = parse_list(components);
        spec.component_provenance = spec
            .components
            .iter()
            .map(|component| ComponentProvenance {
                name: component.clone(),
                source: "manual config".to_string(),
                reason: "selected with --components".to_string(),
            })
            .collect();
    }
    if let Some(name) = &args.name {
        spec.env_name = name.clone();
    }
    if let Some(description) = &args.description {
        spec.description = description.clone();
    }
    if let Some(version) = &args.python_version {
        spec.python_version = version.clone();
    }
    if let Some(systems) = &args.systems {
        spec.supported_systems = parse_list(systems);
    }
    spec.workspace_root = args.workspace_root.clone();

    for item in &args.required_dir {
        spec.add_required_dir(item);
    }
    for item in &args.required_file {
        spec.add_required_file(item);
    }
    for item in &args.source_script {
        push_unique(&mut spec.source_scripts, item);
    }
    for item in &args.env {
        push_unique(&mut spec.env, item);
    }
    if let Some(extra) = &args.with_components {
        for component in parse_list(extra) {
            spec.add_component_with_source(&component, "manual config", "selected with --with");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::manifest::{
        Component, CudaMarkerScan, DependencyRule, Manifest, Profile, RuntimeInference,
        ScriptDiscovery, ScriptRule,
    };
    use super::*;

    #[test]
    fn explicit_cli_values_win_after_project_probing() {
        let target = temp_project("robo-pipeline-explicit");
        fs::write(
            target.join("pyproject.toml"),
            r#"[project]
name = "from-pyproject"
requires-python = "==3.10.*"
dependencies = ["opencv-python"]
"#,
        )
        .unwrap();

        let args = InitArgs {
            target: Some(target.clone()),
            interactive: false,
            list_profiles: false,
            list_components: false,
            stdout: false,
            force: false,
            name: Some("manual-name".to_string()),
            profile: None,
            with_components: Some("cuda-toolkit".to_string()),
            no_probe: false,
            description: None,
            workspace_root: ".".to_string(),
            components: None,
            python_version: Some("3.11".to_string()),
            systems: None,
            required_dir: Vec::new(),
            required_file: Vec::new(),
            source_script: Vec::new(),
            env: Vec::new(),
            robo_nix_url: None,
        };

        let draft = build_draft(&args, &manifest()).unwrap();

        assert_eq!(draft.spec.env_name, "manual-name");
        assert_eq!(draft.spec.python_version, "3.11");
        assert!(draft.spec.components.iter().any(|item| item == "graphics"));
        assert!(
            draft
                .spec
                .components
                .iter()
                .any(|item| item == "cuda-toolkit")
        );

        fs::remove_dir_all(target).unwrap();
    }

    fn temp_project(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn manifest() -> Manifest {
        let mut components = BTreeMap::new();
        components.insert("core".to_string(), component("core runtime"));
        components.insert("graphics".to_string(), component("graphics runtime"));
        components.insert("cuda-toolkit".to_string(), component("CUDA toolkit"));

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "minimal".to_string(),
            Profile {
                description: "minimal".to_string(),
                components: vec!["core".to_string()],
                python_version: "3.12".to_string(),
                supported_systems: vec!["x86_64-linux".to_string()],
                workspace_root: ".".to_string(),
            },
        );

        Manifest {
            components,
            profiles,
            runtime_inference: RuntimeInference {
                default_profile: "minimal".to_string(),
                dependency_rules: vec![DependencyRule {
                    dependencies: vec!["opencv-python".to_string()],
                    components: vec!["graphics".to_string()],
                    note: "OpenCV wheels commonly need graphics runtime libraries".to_string(),
                }],
                workspace_directory_rules: Vec::new(),
                script_discovery: ScriptDiscovery {
                    roots: Vec::new(),
                    names: Vec::new(),
                    prefixes: Vec::new(),
                    daemon_text_contains: Vec::new(),
                    checkout_function: "checkout".to_string(),
                    path_root: "third_party".to_string(),
                },
                script_rules: Vec::<ScriptRule>::new(),
                cuda_marker_scan: CudaMarkerScan {
                    max_depth: 0,
                    max_files: 0,
                    source_extensions: Vec::new(),
                    build_files: Vec::new(),
                    text_contains: Vec::new(),
                    skip_names: Vec::new(),
                    component: "core".to_string(),
                    note: "CUDA marker scan disabled".to_string(),
                },
            },
        }
    }

    fn component(description: &str) -> Component {
        Component {
            category: "test".to_string(),
            description: description.to_string(),
            scaffold_directories: Vec::new(),
            supported_systems: vec!["x86_64-linux".to_string()],
        }
    }
}
