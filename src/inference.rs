use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::error::AppError;

const RUNTIME_INFERENCE_TSV: &str = include_str!("metadata/runtime-inference.tsv");
pub(crate) const KNOWN_COMPONENTS: &[&str] = &[
    "python-uv",
    "native-build",
    "linux-headers",
    "desktop-gl",
    "cuda-toolkit",
];
const KNOWN_CAPABILITIES: &[&str] = &[
    "native-runtime",
    "linux-kernel-headers",
    "desktop-graphics",
    "cuda-build",
];

pub(crate) fn infer_initial_runtime(root: &Path) -> Result<RuntimeInference, AppError> {
    // NOTE: inference is first-bootstrap only; existing robo.nix is canonical.
    let mut inference = RuntimeInference::base();
    let pyproject_path = root.join("pyproject.toml");
    if !pyproject_path.exists() {
        inference.pyproject_status = PyprojectStatus::Missing;
        return Ok(inference);
    }

    let pyproject = fs::read_to_string(&pyproject_path)
        .map_err(|err| AppError::project(format!("failed to read pyproject.toml: {err}")))?;
    let Ok(value) = pyproject.parse::<toml::Value>() else {
        inference.pyproject_status = PyprojectStatus::Invalid;
        return Ok(inference);
    };

    inference.pyproject_status = PyprojectStatus::Read;
    let dependencies = project_dependency_evidence(&value);
    let rules = runtime_rules()?;
    for dependency in dependencies.values() {
        for rule in rules.iter().filter(|rule| rule.package == dependency.name) {
            inference.components.insert(rule.component.clone());
            inference.matches.push(RuntimeMatch {
                package: dependency.name.clone(),
                sources: dependency.sources.iter().cloned().collect(),
                capability: rule.capability.clone(),
                component: rule.component.clone(),
                provenance: rule.provenance.clone(),
                note: rule.note.clone(),
            });
        }
    }

    Ok(inference)
}

pub(crate) fn dependency_names_from_pyproject(root: &Path) -> Result<BTreeSet<String>, AppError> {
    Ok(dependency_evidence_from_pyproject(root)?
        .into_iter()
        .map(|dependency| dependency.name)
        .collect())
}

pub(crate) fn dependency_evidence_from_pyproject(
    root: &Path,
) -> Result<Vec<DependencyEvidence>, AppError> {
    let pyproject_path = root.join("pyproject.toml");
    if !pyproject_path.exists() {
        return Ok(Vec::new());
    }

    let pyproject = fs::read_to_string(&pyproject_path)
        .map_err(|err| AppError::project(format!("failed to read pyproject.toml: {err}")))?;
    let Ok(value) = pyproject.parse::<toml::Value>() else {
        return Ok(Vec::new());
    };
    Ok(project_dependency_evidence(&value)
        .into_values()
        .collect::<Vec<_>>())
}

#[cfg(test)]
fn project_dependency_names(value: &toml::Value) -> BTreeSet<String> {
    project_dependency_evidence(value).into_keys().collect()
}

fn project_dependency_evidence(value: &toml::Value) -> BTreeMap<String, DependencyEvidence> {
    let mut dependencies = BTreeMap::new();

    if let Some(project) = value.get("project").and_then(toml::Value::as_table) {
        if let Some(values) = project.get("dependencies").and_then(toml::Value::as_array) {
            collect_requirement_array(values, "project.dependencies", &mut dependencies);
        }
        if let Some(optional_dependencies) = project
            .get("optional-dependencies")
            .and_then(toml::Value::as_table)
        {
            for (extra, values) in optional_dependencies {
                if let Some(values) = values.as_array() {
                    collect_requirement_array(
                        values,
                        &format!("project.optional-dependencies.{extra}"),
                        &mut dependencies,
                    );
                }
            }
        }
    }

    if let Some(dependency_groups) = value
        .get("dependency-groups")
        .and_then(toml::Value::as_table)
    {
        for (group_name, group) in dependency_groups {
            collect_dependency_group_value(
                group,
                &format!("dependency-groups.{group_name}"),
                &mut dependencies,
            );
        }
    }

    if let Some(dev_dependencies) = value
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("dev-dependencies"))
        .and_then(toml::Value::as_array)
    {
        collect_requirement_array(
            dev_dependencies,
            "tool.uv.dev-dependencies",
            &mut dependencies,
        );
    }

    dependencies
}

fn collect_dependency_group_value(
    value: &toml::Value,
    source: &str,
    dependencies: &mut BTreeMap<String, DependencyEvidence>,
) {
    let Some(items) = value.as_array() else {
        return;
    };
    for item in items {
        if let Some(spec) = item.as_str() {
            collect_requirement(spec, source, dependencies);
        }
    }
}

fn collect_requirement_array(
    values: &[toml::Value],
    source: &str,
    dependencies: &mut BTreeMap<String, DependencyEvidence>,
) {
    for value in values {
        let Some(spec) = value.as_str() else {
            continue;
        };
        collect_requirement(spec, source, dependencies);
    }
}

fn collect_requirement(
    spec: &str,
    source: &str,
    dependencies: &mut BTreeMap<String, DependencyEvidence>,
) {
    if let Some(name) = requirement_name(spec) {
        dependencies
            .entry(name.clone())
            .or_insert_with(|| DependencyEvidence {
                name,
                sources: BTreeSet::new(),
            })
            .sources
            .insert(source.to_string());
    }
}

fn requirement_name(spec: &str) -> Option<String> {
    let mut name = String::new();
    for character in spec.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            if matches!(character, '_' | '.') {
                name.push('-');
            } else {
                name.push(character.to_ascii_lowercase());
            }
        } else {
            break;
        }
    }

    (!name.is_empty()).then_some(name)
}

fn runtime_rules() -> Result<Vec<RuntimeRule>, AppError> {
    let mut rules = Vec::new();
    for (index, line) in RUNTIME_INFERENCE_TSV.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 5 {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} has {} columns, expected 5",
                index + 1,
                columns.len()
            )));
        }
        let capability = columns[1].trim();
        if !KNOWN_CAPABILITIES.contains(&capability) {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} references unknown capability `{capability}`",
                index + 1
            )));
        }
        let component = columns[2].trim();
        if !KNOWN_COMPONENTS.contains(&component) {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} references unknown component `{component}`",
                index + 1
            )));
        }
        let provenance = columns[3].trim();
        let note = columns[4].trim();
        if provenance.is_empty() || note.is_empty() {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} must include provenance and note text",
                index + 1
            )));
        }
        rules.push(RuntimeRule {
            package: normalize_package_name(columns[0].trim()),
            capability: capability.to_string(),
            component: component.to_string(),
            provenance: provenance.to_string(),
            note: note.to_string(),
        });
    }
    Ok(rules)
}

fn normalize_package_name(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            '_' | '.' => '-',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

#[derive(Debug)]
struct RuntimeRule {
    package: String,
    capability: String,
    component: String,
    provenance: String,
    note: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct DependencyEvidence {
    pub(crate) name: String,
    pub(crate) sources: BTreeSet<String>,
}

#[derive(Debug)]
pub(crate) struct RuntimeMatch {
    pub(crate) package: String,
    pub(crate) sources: Vec<String>,
    pub(crate) capability: String,
    pub(crate) component: String,
    pub(crate) provenance: String,
    pub(crate) note: String,
}

#[derive(Debug)]
pub(crate) struct RuntimeInference {
    pub(crate) components: BTreeSet<String>,
    pub(crate) matches: Vec<RuntimeMatch>,
    pub(crate) pyproject_status: PyprojectStatus,
}

impl RuntimeInference {
    fn base() -> Self {
        Self {
            components: BTreeSet::from(["python-uv".to_string()]),
            matches: Vec::new(),
            pyproject_status: PyprojectStatus::Missing,
        }
    }
}

#[derive(Debug)]
pub(crate) enum PyprojectStatus {
    Missing,
    Invalid,
    Read,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_names_are_normalized() {
        assert_eq!(
            requirement_name("opencv_python>=4").as_deref(),
            Some("opencv-python")
        );
        assert_eq!(
            requirement_name("torch[dev] == 2").as_deref(),
            Some("torch")
        );
    }

    #[test]
    fn pyproject_dependency_names_include_optional_and_uv_groups() {
        let value = r#"
[project]
dependencies = [
  "torch>=2",
]

[project.optional-dependencies]
sim = [
  "dm-control",
  "gymnasium-robotics",
]

[dependency-groups]
dev = [
  "mujoco",
  { include-group = "lint" },
]

[tool.uv]
dev-dependencies = [
  "opencv_contrib_python>=4",
]
"#
        .parse::<toml::Value>()
        .unwrap();

        let names = project_dependency_names(&value);

        assert!(names.contains("torch"));
        assert!(names.contains("dm-control"));
        assert!(names.contains("gymnasium-robotics"));
        assert!(names.contains("mujoco"));
        assert!(names.contains("opencv-contrib-python"));
        assert!(!names.contains("include-group"));
    }

    #[test]
    fn pyproject_dependency_evidence_tracks_sources() {
        let value = r#"
[project]
dependencies = ["torch"]

[project.optional-dependencies]
sim = ["torch", "mujoco"]

[dependency-groups]
dev = ["mujoco"]
"#
        .parse::<toml::Value>()
        .unwrap();

        let evidence = project_dependency_evidence(&value);

        assert_eq!(
            evidence
                .get("torch")
                .unwrap()
                .sources
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "project.dependencies".to_string(),
                "project.optional-dependencies.sim".to_string()
            ]
        );
        assert_eq!(
            evidence
                .get("mujoco")
                .unwrap()
                .sources
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "dependency-groups.dev".to_string(),
                "project.optional-dependencies.sim".to_string()
            ]
        );
    }

    #[test]
    fn runtime_rules_have_capabilities_and_provenance() {
        let rules = runtime_rules().unwrap();

        assert!(rules.iter().any(|rule| {
            rule.package == "mujoco"
                && rule.capability == "desktop-graphics"
                && rule.component == "desktop-gl"
                && !rule.provenance.is_empty()
        }));
    }
}
