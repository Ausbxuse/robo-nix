use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::manifest::Manifest;

// Flat facts from project probing. Keep policy decisions in spec/pipeline, not here.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProbeResult {
    pub(super) env_name: Option<String>,
    pub(super) python_version: Option<ProbeValue>,
    pub(super) cuda_wheel_version: Option<ProbeValue>,
    pub(super) requirements: Vec<ProbeRequirement>,
    pub(super) components: Vec<ProbeComponent>,
    pub(super) required_dirs: Vec<String>,
    pub(super) notes: Vec<String>,
}

impl ProbeResult {
    fn add_component(&mut self, name: &str, note: &str) {
        self.components.push(ProbeComponent {
            name: name.to_string(),
            note: note.to_string(),
        });
    }

    fn add_requirements(&mut self, requirements: &[String], note: &str) {
        for requirement in requirements {
            self.requirements.push(ProbeRequirement {
                id: requirement.clone(),
                note: note.to_string(),
            });
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProbeValue {
    pub(super) value: String,
    pub(super) note: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProbeComponent {
    pub(super) name: String,
    pub(super) note: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProbeRequirement {
    pub(super) id: String,
    pub(super) note: String,
}

pub(super) fn probe_project(target: &Path, manifest: &Manifest) -> ProbeResult {
    let mut probe = ProbeResult::default();
    let pyproject = target.join("pyproject.toml");
    if let Ok(text) = fs::read_to_string(&pyproject) {
        probe_pyproject_name(&text, &mut probe);
        probe_python_version(&text, &mut probe);
        probe_dependencies(&text, manifest, &mut probe);
    }
    probe_cuda_lock_version(target, &mut probe);
    probe
}

fn probe_cuda_lock_version(target: &Path, probe: &mut ProbeResult) {
    let lock_path = target.join("uv.lock");
    let Ok(lock) = fs::read_to_string(lock_path) else {
        return;
    };
    if let Some(version) = crate::runtime::infer_cuda_wheel_version_from_uv_lock_text(&lock) {
        probe.cuda_wheel_version = Some(ProbeValue {
            value: version.clone(),
            note: format!("cudaWheelVersion={version}: inferred from uv.lock"),
        });
    }
}

fn probe_pyproject_name(text: &str, probe: &mut ProbeResult) {
    if let Some(name) = crate::pyproject::project_name(text) {
        probe.env_name = Some(name);
    }
}

fn probe_python_version(text: &str, probe: &mut ProbeResult) {
    if let Some(version) = crate::exact_python_requirement(text) {
        probe.python_version = Some(ProbeValue {
            value: version.clone(),
            note: format!("python {version}: pyproject.toml requires-python"),
        });
        return;
    }

    if let Some(raw) = crate::pyproject::python_requirement(text)
        && let Some(version) = infer_python_version(&raw)
    {
        probe.python_version = Some(ProbeValue {
            value: version.to_string(),
            note: format!("python {version}: pyproject.toml requires-python"),
        });
    }
}

fn infer_python_version(raw: &str) -> Option<&str> {
    let mut first = None;
    let mut exact = None;
    let mut offset = 0;

    while offset < raw.len() {
        let rest = &raw[offset..];
        let Some(start_delta) = rest.find(|ch: char| ch.is_ascii_digit()) else {
            break;
        };
        let start = offset + start_delta;
        let end = raw[start..]
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '*'))
            .map_or(raw.len(), |end_delta| start + end_delta);
        let token = &raw[start..end];
        let version = token.strip_suffix(".*").unwrap_or(token);
        let parts = version.split('.').collect::<Vec<_>>();

        if matches!(parts.len(), 2 | 3)
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        {
            first.get_or_insert(version);

            let prefix = raw[..start].trim_end();
            if prefix.ends_with("===") || (prefix.ends_with("==") && !prefix.ends_with("!=")) {
                exact = Some(version);
                break;
            }
        }

        offset = end + 1;
    }

    exact.or(first)
}

fn probe_dependencies(text: &str, manifest: &Manifest, probe: &mut ProbeResult) {
    let deps = crate::pyproject::dependency_names(text);
    for rule in &manifest.runtime_inference.dependency_rules {
        if has_dep(&deps, &rule.dependencies) {
            for component in &rule.components {
                probe.add_component(component, &rule.note);
            }
            if !rule.requires.is_empty() {
                probe.add_requirements(&rule.requires, &rule.note);
            }
        }
    }
    for rule in &manifest.runtime_inference.compound_dependency_rules {
        if rule
            .dependencies_all
            .iter()
            .all(|group| has_dep(&deps, group))
        {
            for component in &rule.components {
                probe.add_component(component, &rule.note);
            }
            if !rule.requires.is_empty() {
                probe.add_requirements(&rule.requires, &rule.note);
            }
        }
    }
}

fn has_dep(dependencies: &BTreeSet<String>, names: &[String]) -> bool {
    crate::pyproject::has_dependency_name(dependencies, names.iter().map(String::as_str))
}
