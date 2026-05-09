use std::collections::BTreeSet;
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
    let dependencies = project_dependency_names(&value);
    let rules = runtime_rules()?;
    for dependency in dependencies {
        for rule in rules.iter().filter(|rule| rule.package == dependency) {
            inference.components.insert(rule.component.clone());
            inference.matches.push(RuntimeMatch {
                package: dependency.clone(),
                component: rule.component.clone(),
                note: rule.note.clone(),
            });
        }
    }

    Ok(inference)
}

fn project_dependency_names(value: &toml::Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(project) = value.get("project").and_then(toml::Value::as_table) else {
        return names;
    };
    let Some(dependencies) = project.get("dependencies").and_then(toml::Value::as_array) else {
        return names;
    };

    for dependency in dependencies {
        let Some(spec) = dependency.as_str() else {
            continue;
        };
        if let Some(name) = requirement_name(spec) {
            names.insert(name);
        }
    }

    names
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
        if columns.len() != 3 {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} has {} columns, expected 3",
                index + 1,
                columns.len()
            )));
        }
        let component = columns[1].trim();
        if !KNOWN_COMPONENTS.contains(&component) {
            return Err(AppError::project(format!(
                "src/metadata/runtime-inference.tsv line {} references unknown component `{component}`",
                index + 1
            )));
        }
        rules.push(RuntimeRule {
            package: normalize_package_name(columns[0].trim()),
            component: component.to_string(),
            note: columns[2].trim().to_string(),
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
    component: String,
    note: String,
}

#[derive(Debug)]
pub(crate) struct RuntimeMatch {
    pub(crate) package: String,
    pub(crate) component: String,
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
}
