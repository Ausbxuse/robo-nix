use std::collections::BTreeSet;
use std::mem;

use super::manifest::Manifest;
use super::probe::ProbeResult;

#[derive(Clone)]
pub(super) struct ProjectSpec {
    pub(super) profile_name: String,
    pub(super) env_name: String,
    pub(super) description: String,
    pub(super) requirements: Vec<RuntimeRequirement>,
    pub(super) components: Vec<String>,
    pub(super) cuda_wheel_version: Option<String>,
    pub(super) python_version: String,
    pub(super) supported_systems: Vec<String>,
    pub(super) workspace_root: String,
    pub(super) required_dirs: Vec<String>,
    pub(super) required_files: Vec<String>,
    pub(super) source_scripts: Vec<String>,
    pub(super) env: Vec<String>,
    pub(super) probe_notes: Vec<String>,
    pub(super) component_provenance: Vec<ComponentProvenance>,
    pub(super) suggestions: Vec<InferenceSuggestion>,
    pub(super) component_suggestions: Vec<ComponentSuggestion>,
}

impl ProjectSpec {
    pub(super) fn from_profile(name: &str, manifest: &Manifest) -> Result<Self, String> {
        let profile = manifest
            .profiles
            .get(name)
            .ok_or_else(|| format!("unknown profile: {name}"))?;
        Ok(Self {
            profile_name: name.to_string(),
            env_name: "project".to_string(),
            description: profile.description.clone(),
            requirements: profile
                .components
                .iter()
                .flat_map(|component| {
                    manifest
                        .components
                        .get(component)
                        .into_iter()
                        .flat_map(|metadata| &metadata.provides)
                        .map(|requirement| RuntimeRequirement {
                            id: requirement.clone(),
                        })
                })
                .collect(),
            components: profile.components.clone(),
            cuda_wheel_version: None,
            python_version: profile.python_version.clone(),
            supported_systems: profile.supported_systems.clone(),
            workspace_root: profile.workspace_root.clone(),
            required_dirs: Vec::new(),
            required_files: Vec::new(),
            source_scripts: Vec::new(),
            env: Vec::new(),
            probe_notes: Vec::new(),
            component_provenance: profile
                .components
                .iter()
                .map(|component| ComponentProvenance {
                    name: component.clone(),
                    source: "profile".to_string(),
                    reason: format!("selected by the `{name}` profile"),
                })
                .collect(),
            suggestions: Vec::new(),
            component_suggestions: Vec::new(),
        })
    }

    pub(super) fn add_component(&mut self, component: &str, note: impl Into<String>) {
        let note = note.into();
        let source = inference_source(&note);
        self.add_component_with_source(component, source, note);
    }

    pub(super) fn add_component_with_source(
        &mut self,
        component: &str,
        source: &str,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        if !self.components.iter().any(|item| item == component) {
            self.components.push(component.to_string());
            self.probe_notes.push(reason.clone());
        }
        if !self
            .component_provenance
            .iter()
            .any(|item| item.name == component)
        {
            self.component_provenance.push(ComponentProvenance {
                name: component.to_string(),
                source: source.to_string(),
                reason,
            });
        }
    }

    pub(super) fn add_requirement(
        &mut self,
        manifest: &Manifest,
        id: &str,
        source: &str,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        if !self.requirements.iter().any(|item| item.id == id) {
            self.requirements.push(RuntimeRequirement {
                id: id.to_string(),
            });
        }
        for component in providers_for(manifest, id) {
            if !self.components.iter().any(|item| item == component) {
                self.components.push(component.to_string());
            }
            if !self
                .component_provenance
                .iter()
                .any(|item| item.name == component)
            {
                self.component_provenance.push(ComponentProvenance {
                    name: component.to_string(),
                    source: source.to_string(),
                    reason: format!("provides `{id}`: {reason}"),
                });
            }
        }
        push_unique(&mut self.probe_notes, &reason);
    }

    pub(super) fn add_required_dir(&mut self, path: &str) {
        push_unique(&mut self.required_dirs, path);
    }

    pub(super) fn add_required_file(&mut self, path: &str) {
        push_unique(&mut self.required_files, path);
    }

    pub(super) fn add_suggestion(&mut self, kind: &str, path: &str, reason: impl Into<String>) {
        if !self
            .suggestions
            .iter()
            .any(|item| item.kind == kind && item.path == path)
        {
            self.suggestions.push(InferenceSuggestion {
                kind: kind.to_string(),
                path: path.to_string(),
                reason: reason.into(),
            });
        }
    }

    pub(super) fn add_source_script(&mut self, path: &str) {
        push_unique(&mut self.source_scripts, path);
    }

    pub(super) fn add_component_suggestion(
        &mut self,
        component: &str,
        evidence: &str,
        reason: impl Into<String>,
    ) {
        if self.components.iter().any(|item| item == component)
            || self
                .component_suggestions
                .iter()
                .any(|item| item.name == component && item.evidence == evidence)
        {
            return;
        }
        self.component_suggestions.push(ComponentSuggestion {
            name: component.to_string(),
            evidence: evidence.to_string(),
            reason: reason.into(),
        });
    }

    pub(super) fn apply_probe(&mut self, manifest: &Manifest, probe: ProbeResult) {
        if self.env_name == "project" {
            if let Some(name) = probe.env_name {
                self.env_name = name;
            }
        }
        if let Some(python) = probe.python_version {
            self.python_version = python.value;
            self.probe_notes.push(python.note);
        }
        if let Some(cuda) = probe.cuda_wheel_version {
            self.cuda_wheel_version = Some(cuda.value);
            self.probe_notes.push(cuda.note);
        }
        for component in probe.components {
            self.add_component(&component.name, component.note);
        }
        for requirement in probe.requirements {
            self.add_requirement(
                manifest,
                &requirement.id,
                inference_source(&requirement.note),
                requirement.note,
            );
        }
        for path in probe.required_dirs {
            self.add_required_dir(&path);
        }
        self.probe_notes.extend(probe.notes);
        for suggestion in probe.suggestions {
            self.add_suggestion(&suggestion.kind, &suggestion.path, suggestion.reason);
        }
        for suggestion in probe.component_suggestions {
            self.add_component_suggestion(
                &suggestion.name,
                &suggestion.evidence,
                suggestion.reason,
            );
        }
    }
}

#[derive(Clone)]
pub(super) struct ComponentProvenance {
    pub(super) name: String,
    pub(super) source: String,
    pub(super) reason: String,
}

#[derive(Clone)]
pub(super) struct RuntimeRequirement {
    pub(super) id: String,
}

#[derive(Clone)]
pub(super) struct InferenceSuggestion {
    pub(super) kind: String,
    pub(super) path: String,
    pub(super) reason: String,
}

#[derive(Clone)]
pub(super) struct ComponentSuggestion {
    pub(super) name: String,
    pub(super) evidence: String,
    pub(super) reason: String,
}

fn inference_source(note: &str) -> &'static str {
    let lower = note.to_ascii_lowercase();
    if lower.contains("pyproject.toml")
        || lower.contains("wheels")
        || lower.contains("packages")
        || lower.contains("workflows")
    {
        "pyproject inference"
    } else if lower.contains("workspace") || lower.contains("bootstrap script") {
        "workspace inference"
    } else {
        "inference"
    }
}

pub(super) fn parse_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn push_unique(items: &mut Vec<String>, value: &str) {
    if !items.iter().any(|item| item == value) {
        items.push(value.to_string());
    }
}

pub(super) fn dedupe_all(spec: &mut ProjectSpec) {
    spec.requirements
        .sort_by(|left, right| left.id.cmp(&right.id));
    spec.requirements.dedup_by(|left, right| left.id == right.id);
    spec.components = dedupe(mem::take(&mut spec.components));
    spec.supported_systems = dedupe(mem::take(&mut spec.supported_systems));
    spec.required_dirs = dedupe(mem::take(&mut spec.required_dirs));
    spec.required_files = dedupe(mem::take(&mut spec.required_files));
    spec.source_scripts = dedupe(mem::take(&mut spec.source_scripts));
    spec.env = dedupe(mem::take(&mut spec.env));
    spec.probe_notes = dedupe(mem::take(&mut spec.probe_notes));
    spec.component_provenance.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    spec.suggestions.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    spec.component_suggestions.retain(|suggestion| {
        !spec
            .components
            .iter()
            .any(|component| component == &suggestion.name)
    });
    spec.component_suggestions.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.evidence.cmp(&right.evidence))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    spec.component_suggestions.dedup_by(|left, right| {
        left.name == right.name && left.evidence == right.evidence && left.reason == right.reason
    });
}

pub(super) fn providers_for<'a>(manifest: &'a Manifest, requirement: &str) -> Vec<&'a str> {
    if requirement.starts_with("host.") {
        return Vec::new();
    }
    manifest
        .components
        .iter()
        .filter(|(_, component)| component.provides.iter().any(|item| item == requirement))
        .map(|(name, _)| name.as_str())
        .collect()
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}
