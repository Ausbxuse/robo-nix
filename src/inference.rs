use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

const RUNTIME_INFERENCE_TSV: &str = include_str!("metadata/runtime-inference.tsv");
const MAX_LOCAL_METADATA_VISITS: usize = 128;
pub(crate) const KNOWN_COMPONENTS: &[&str] = &[
    "python-uv",
    "native-build",
    "linux-headers",
    "camera-usb",
    "desktop-gl",
    "qt6",
    "cuda-toolkit",
];
const KNOWN_CAPABILITIES: &[&str] = &[
    "native-runtime",
    "linux-kernel-headers",
    "usb-camera-runtime",
    "desktop-graphics",
    "qt-runtime",
    "cuda-build",
];

pub(crate) fn infer_initial_runtime(root: &Path) -> Result<RuntimeInference, AppError> {
    // NOTE: inference is first-bootstrap only; existing robo.nix is canonical.
    let mut inference = RuntimeInference::base();
    let scan = dependency_scan_from_pyproject(root)?;
    inference.pyproject_status = scan.pyproject_status;
    inference.diagnostics = scan.diagnostics;

    let dependencies = scan.dependencies;
    let rules = runtime_rules()?;
    let mut matched_packages = BTreeSet::new();
    for dependency in dependencies.values() {
        for rule in rules.iter().filter(|rule| rule.package == dependency.name) {
            inference.components.insert(rule.component.clone());
            matched_packages.insert(dependency.name.clone());
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

    let unresolved_remote_dependencies = scan
        .remote_dependencies
        .difference(&matched_packages)
        .cloned()
        .collect::<Vec<_>>();
    if !unresolved_remote_dependencies.is_empty() {
        inference.diagnostics.push(InferenceDiagnostic {
            summary: format!(
                "static inference skipped remote package metadata for {} dependencies",
                unresolved_remote_dependencies.len()
            ),
            detail: Some(format!(
                "uv will resolve them later; add components to `robo.nix` if a remote transitive dependency needs native libraries. examples: {}",
                unresolved_remote_dependencies
                    .iter()
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        });
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
    Ok(dependency_scan_from_pyproject(root)?
        .dependencies
        .into_values()
        .collect::<Vec<_>>())
}

fn dependency_scan_from_pyproject(root: &Path) -> Result<DependencyScan, AppError> {
    let pyproject_path = root.join("pyproject.toml");
    if !pyproject_path.exists() {
        return Ok(DependencyScan {
            pyproject_status: PyprojectStatus::Missing,
            ..DependencyScan::default()
        });
    }

    let pyproject = fs::read_to_string(&pyproject_path)
        .map_err(|err| AppError::project(format!("failed to read pyproject.toml: {err}")))?;
    let Ok(value) = pyproject.parse::<toml::Value>() else {
        return Ok(DependencyScan {
            pyproject_status: PyprojectStatus::Invalid,
            ..DependencyScan::default()
        });
    };

    let mut resolver = DependencyResolver::new(root);
    resolver.visit_pyproject(
        root,
        &value,
        PyprojectScope::Root,
        &BTreeSet::new(),
        &BTreeMap::new(),
    );
    let mut scan = DependencyScan {
        pyproject_status: PyprojectStatus::Read,
        dependencies: resolver.dependencies,
        diagnostics: resolver.diagnostics,
        remote_dependencies: resolver.remote_dependencies,
    };
    collect_uv_lock_dependencies(root, &mut scan);
    Ok(scan)
}

#[cfg(test)]
fn project_dependency_names(value: &toml::Value) -> BTreeSet<String> {
    project_dependency_evidence(value).into_keys().collect()
}

#[cfg(test)]
fn project_dependency_evidence(value: &toml::Value) -> BTreeMap<String, DependencyEvidence> {
    let mut collector = DependencyCollector::default();
    collector.collect_pyproject_requirements(
        value,
        "pyproject.toml",
        PyprojectScope::Root,
        &BTreeSet::new(),
    );
    collector.dependencies
}

fn dependency_source(source_prefix: &str, section: &str) -> String {
    if source_prefix == "pyproject.toml" {
        section.to_string()
    } else {
        format!("{source_prefix}:{section}")
    }
}

#[derive(Default)]
struct DependencyCollector {
    dependencies: BTreeMap<String, DependencyEvidence>,
}

impl DependencyCollector {
    fn collect_pyproject_requirements(
        &mut self,
        value: &toml::Value,
        source_prefix: &str,
        scope: PyprojectScope,
        selected_extras: &BTreeSet<String>,
    ) -> Vec<Requirement> {
        let mut requirements = Vec::new();

        if let Some(project) = value.get("project").and_then(toml::Value::as_table) {
            if let Some(values) = project.get("dependencies").and_then(toml::Value::as_array) {
                self.collect_requirement_array(
                    values,
                    &dependency_source(source_prefix, "project.dependencies"),
                    &mut requirements,
                );
            }
            if let Some(optional_dependencies) = project
                .get("optional-dependencies")
                .and_then(toml::Value::as_table)
            {
                for (extra, values) in optional_dependencies {
                    let extra = normalize_package_name(extra);
                    if scope == PyprojectScope::Local && !selected_extras.contains(&extra) {
                        continue;
                    }
                    if let Some(values) = values.as_array() {
                        self.collect_requirement_array(
                            values,
                            &dependency_source(
                                source_prefix,
                                &format!("project.optional-dependencies.{extra}"),
                            ),
                            &mut requirements,
                        );
                    }
                }
            }
        }

        if scope == PyprojectScope::Root {
            if let Some(dependency_groups) = value
                .get("dependency-groups")
                .and_then(toml::Value::as_table)
            {
                for group_name in dependency_groups.keys() {
                    self.collect_dependency_group_by_name(
                        dependency_groups,
                        group_name,
                        source_prefix,
                        &mut requirements,
                        &mut BTreeSet::new(),
                    );
                }
            }

            if let Some(dev_dependencies) = value
                .get("tool")
                .and_then(|tool| tool.get("uv"))
                .and_then(|uv| uv.get("dev-dependencies"))
                .and_then(toml::Value::as_array)
            {
                self.collect_requirement_array(
                    dev_dependencies,
                    &dependency_source(source_prefix, "tool.uv.dev-dependencies"),
                    &mut requirements,
                );
            }
        }

        requirements
    }

    fn collect_dependency_group_value(
        &mut self,
        value: &toml::Value,
        source: &str,
        source_prefix: &str,
        dependency_groups: &toml::map::Map<String, toml::Value>,
        requirements: &mut Vec<Requirement>,
        seen_groups: &mut BTreeSet<String>,
    ) {
        let Some(items) = value.as_array() else {
            return;
        };
        for item in items {
            if let Some(spec) = item.as_str() {
                self.collect_requirement(spec, source, requirements);
            } else if let Some(included_group) = item
                .as_table()
                .and_then(|table| table.get("include-group"))
                .and_then(toml::Value::as_str)
            {
                self.collect_dependency_group_by_name(
                    dependency_groups,
                    included_group,
                    source_prefix,
                    requirements,
                    seen_groups,
                );
            }
        }
    }

    fn collect_dependency_group_by_name(
        &mut self,
        dependency_groups: &toml::map::Map<String, toml::Value>,
        group_name: &str,
        source_prefix: &str,
        requirements: &mut Vec<Requirement>,
        seen_groups: &mut BTreeSet<String>,
    ) {
        if !seen_groups.insert(group_name.to_string()) {
            return;
        }
        let Some(group) = dependency_groups.get(group_name) else {
            return;
        };
        self.collect_dependency_group_value(
            group,
            &dependency_source(source_prefix, &format!("dependency-groups.{group_name}")),
            source_prefix,
            dependency_groups,
            requirements,
            seen_groups,
        );
    }

    fn collect_requirement_array(
        &mut self,
        values: &[toml::Value],
        source: &str,
        requirements: &mut Vec<Requirement>,
    ) {
        for value in values {
            let Some(spec) = value.as_str() else {
                continue;
            };
            self.collect_requirement(spec, source, requirements);
        }
    }

    fn collect_requirement(
        &mut self,
        spec: &str,
        source: &str,
        requirements: &mut Vec<Requirement>,
    ) {
        if let Some(requirement) = parse_requirement(spec) {
            add_dependency_evidence(&mut self.dependencies, &requirement.name, source);
            requirements.push(requirement);
        }
    }
}

fn collect_uv_lock_dependencies(root: &Path, scan: &mut DependencyScan) {
    let path = root.join("uv.lock");
    if !path.exists() {
        return;
    }

    let lock = match fs::read_to_string(&path) {
        Ok(lock) => lock,
        Err(err) => {
            scan.diagnostics.push(InferenceDiagnostic {
                summary: "could not inspect uv.lock for resolved package metadata".to_string(),
                detail: Some(format!("failed to read uv.lock: {err}")),
            });
            return;
        }
    };
    for name in uv_lock_package_names(&lock) {
        add_dependency_evidence(
            &mut scan.dependencies,
            &normalize_package_name(&name),
            "uv.lock",
        );
    }
}

fn uv_lock_package_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_package = false;

    for line in text.lines().map(str::trim) {
        if line == "[[package]]" {
            in_package = true;
            continue;
        }
        if line.starts_with('[') {
            in_package = false;
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(name) = line.strip_prefix("name = ").and_then(extract_quoted) else {
            continue;
        };
        names.push(name.to_string());
        in_package = false;
    }

    names
}

fn extract_quoted(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    value.split_once('"').map(|(quoted, _)| quoted)
}

fn add_dependency_evidence(
    dependencies: &mut BTreeMap<String, DependencyEvidence>,
    name: &str,
    source: &str,
) {
    dependencies
        .entry(name.to_string())
        .or_insert_with(|| DependencyEvidence {
            name: name.to_string(),
            sources: BTreeSet::new(),
        })
        .sources
        .insert(source.to_string());
}

#[derive(Debug)]
struct DependencyScan {
    pyproject_status: PyprojectStatus,
    dependencies: BTreeMap<String, DependencyEvidence>,
    diagnostics: Vec<InferenceDiagnostic>,
    remote_dependencies: BTreeSet<String>,
}

impl Default for DependencyScan {
    fn default() -> Self {
        Self {
            pyproject_status: PyprojectStatus::Missing,
            dependencies: BTreeMap::new(),
            diagnostics: Vec::new(),
            remote_dependencies: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PyprojectScope {
    Root,
    Local,
}

#[derive(Clone, Debug)]
struct Requirement {
    name: String,
    extras: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct LocalSource {
    path: PathBuf,
    origin: String,
}

struct DependencyResolver<'a> {
    root: &'a Path,
    dependencies: BTreeMap<String, DependencyEvidence>,
    diagnostics: Vec<InferenceDiagnostic>,
    remote_dependencies: BTreeSet<String>,
    visited_local_metadata: BTreeSet<String>,
}

impl<'a> DependencyResolver<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            dependencies: BTreeMap::new(),
            diagnostics: Vec::new(),
            remote_dependencies: BTreeSet::new(),
            visited_local_metadata: BTreeSet::new(),
        }
    }

    fn visit_pyproject(
        &mut self,
        project_dir: &Path,
        value: &toml::Value,
        scope: PyprojectScope,
        selected_extras: &BTreeSet<String>,
        inherited_sources: &BTreeMap<String, LocalSource>,
    ) {
        if self.visited_local_metadata.len() > MAX_LOCAL_METADATA_VISITS {
            self.diagnostics.push(InferenceDiagnostic {
                summary: "static inference stopped after too many local pyproject files".to_string(),
                detail: Some(format!(
                    "visited more than {MAX_LOCAL_METADATA_VISITS} local source metadata files; add missing runtime components to `robo.nix` manually."
                )),
            });
            return;
        }

        let source_prefix = self.source_prefix(project_dir);
        let mut collector = DependencyCollector {
            dependencies: std::mem::take(&mut self.dependencies),
        };
        let requirements =
            collector.collect_pyproject_requirements(value, &source_prefix, scope, selected_extras);
        self.dependencies = collector.dependencies;

        let mut sources = inherited_sources.clone();
        sources.extend(local_sources_from_pyproject(
            value,
            project_dir,
            &source_prefix,
        ));

        for requirement in requirements {
            if let Some(source) = sources.get(&requirement.name) {
                self.visit_local_source(&requirement, source, &sources);
            } else {
                self.remote_dependencies.insert(requirement.name);
            }
        }
    }

    fn visit_local_source(
        &mut self,
        requirement: &Requirement,
        source: &LocalSource,
        inherited_sources: &BTreeMap<String, LocalSource>,
    ) {
        let pyproject_path = source.path.join("pyproject.toml");
        let selected_extras = requirement.extras.clone();
        let visit_key = format!(
            "{}:{}",
            pyproject_path.display(),
            selected_extras
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        );
        if !self.visited_local_metadata.insert(visit_key) {
            return;
        }

        if !source.path.exists() {
            self.diagnostics.push(InferenceDiagnostic {
                summary: format!(
                    "could not inspect local source `{}` for `{}`",
                    self.display_path(&source.path),
                    requirement.name
                ),
                detail: Some(format!(
                    "`{}` points at a path that does not exist; add needed components to `robo.nix` if this dependency has native runtime requirements.",
                    source.origin
                )),
            });
            return;
        }
        if !pyproject_path.exists() {
            self.diagnostics.push(InferenceDiagnostic {
                summary: format!(
                    "could not inspect local source `{}` for `{}`",
                    self.display_path(&source.path),
                    requirement.name
                ),
                detail: Some(
                    "no pyproject.toml was found there; add needed components to `robo.nix` if this dependency has native runtime requirements."
                        .to_string(),
                ),
            });
            return;
        }

        let pyproject = match fs::read_to_string(&pyproject_path) {
            Ok(pyproject) => pyproject,
            Err(err) => {
                self.diagnostics.push(InferenceDiagnostic {
                    summary: format!(
                        "could not inspect local source `{}` for `{}`",
                        self.display_path(&source.path),
                        requirement.name
                    ),
                    detail: Some(format!("failed to read pyproject.toml: {err}")),
                });
                return;
            }
        };
        let value = match pyproject.parse::<toml::Value>() {
            Ok(value) => value,
            Err(_) => {
                self.diagnostics.push(InferenceDiagnostic {
                    summary: format!(
                        "could not inspect local source `{}` for `{}`",
                        self.display_path(&source.path),
                        requirement.name
                    ),
                    detail: Some(
                        "local pyproject.toml is invalid TOML; add needed components to `robo.nix` manually."
                            .to_string(),
                    ),
                });
                return;
            }
        };

        let available_extras = optional_dependency_names(&value);
        for extra in &selected_extras {
            if !available_extras.contains(extra) {
                self.diagnostics.push(InferenceDiagnostic {
                    summary: format!(
                        "local source `{}` does not define extra `{}` for `{}`",
                        self.display_path(&source.path),
                        extra,
                        requirement.name
                    ),
                    detail: Some(
                        "static inference skipped that extra; check the dependency spelling or add needed components to `robo.nix` manually."
                            .to_string(),
                    ),
                });
            }
        }

        self.visit_pyproject(
            &source.path,
            &value,
            PyprojectScope::Local,
            &selected_extras,
            inherited_sources,
        );
    }

    fn source_prefix(&self, project_dir: &Path) -> String {
        if project_dir == self.root {
            "pyproject.toml".to_string()
        } else {
            format!("{}/pyproject.toml", self.display_path(project_dir))
        }
    }

    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(self.root)
            .map(|path| format!("./{}", path.display()))
            .unwrap_or_else(|_| path.display().to_string())
    }
}

fn local_sources_from_pyproject(
    value: &toml::Value,
    project_dir: &Path,
    source_prefix: &str,
) -> BTreeMap<String, LocalSource> {
    let mut sources = BTreeMap::new();
    let Some(source_table) = value
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("sources"))
        .and_then(toml::Value::as_table)
    else {
        return sources;
    };

    for (package, value) in source_table {
        if let Some(path) = local_source_path(value, project_dir) {
            sources.insert(
                normalize_package_name(package),
                LocalSource {
                    path,
                    origin: format!("{source_prefix}:tool.uv.sources.{package}"),
                },
            );
        }
    }

    sources
}

fn local_source_path(value: &toml::Value, project_dir: &Path) -> Option<PathBuf> {
    if let Some(table) = value.as_table() {
        return table
            .get("path")
            .and_then(toml::Value::as_str)
            .map(|path| resolve_local_path(project_dir, path));
    }
    value.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            value
                .as_table()
                .and_then(|table| table.get("path"))
                .and_then(toml::Value::as_str)
                .map(|path| resolve_local_path(project_dir, path))
        })
    })
}

fn resolve_local_path(project_dir: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

fn optional_dependency_names(value: &toml::Value) -> BTreeSet<String> {
    if let Some(project) = value.get("project").and_then(toml::Value::as_table) {
        return project
            .get("optional-dependencies")
            .and_then(toml::Value::as_table)
            .map(|extras| {
                extras
                    .keys()
                    .map(|extra| normalize_package_name(extra))
                    .collect()
            })
            .unwrap_or_default();
    }
    BTreeSet::new()
}

#[cfg(test)]
fn requirement_name(spec: &str) -> Option<String> {
    parse_requirement(spec).map(|requirement| requirement.name)
}

fn parse_requirement(spec: &str) -> Option<Requirement> {
    let mut name = String::new();
    let trimmed = spec.trim();
    let mut name_end = 0;
    for (index, character) in trimmed.char_indices() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            if matches!(character, '_' | '.') {
                name.push('-');
            } else {
                name.push(character.to_ascii_lowercase());
            }
            name_end = index + character.len_utf8();
        } else {
            break;
        }
    }

    if name.is_empty() {
        return None;
    }

    let mut extras = BTreeSet::new();
    let rest = trimmed[name_end..].trim_start();
    if let Some(rest) = rest.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            for extra in rest[..end].split(',') {
                let extra = normalize_package_name(extra.trim());
                if !extra.is_empty() {
                    extras.insert(extra);
                }
            }
        }
    }

    Some(Requirement { name, extras })
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
    pub(crate) diagnostics: Vec<InferenceDiagnostic>,
    pub(crate) pyproject_status: PyprojectStatus,
}

impl RuntimeInference {
    fn base() -> Self {
        Self {
            components: BTreeSet::from(["python-uv".to_string()]),
            matches: Vec::new(),
            diagnostics: Vec::new(),
            pyproject_status: PyprojectStatus::Missing,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InferenceDiagnostic {
    pub(crate) summary: String,
    pub(crate) detail: Option<String>,
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
    use std::env;
    use std::fs;
    use std::path::PathBuf;

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
        assert_eq!(
            parse_requirement("robot_driver_stack[full,input]>=1")
                .unwrap()
                .extras
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["full".to_string(), "input".to_string()]
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

    #[test]
    fn inference_follows_local_path_dependency_extras() {
        let root = temp_project("local-path-extras");
        fs::create_dir_all(root.join("vendor/robot_stack")).unwrap();
        fs::create_dir_all(root.join("vendor/input_driver")).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = []

[dependency-groups]
runtime = [
  "robot-stack[full]",
]

[tool.uv.sources]
robot-stack = { path = "vendor/robot_stack" }
evdev = { path = "vendor/input_driver" }
"#,
        )
        .unwrap();
        fs::write(
            root.join("vendor/robot_stack/pyproject.toml"),
            r#"[project]
name = "robot-stack"
dependencies = [
  "torch",
]

[project.optional-dependencies]
full = [
  "evdev; sys_platform == 'linux'",
]
"#,
        )
        .unwrap();
        fs::write(
            root.join("vendor/input_driver/pyproject.toml"),
            r#"[project]
name = "evdev"
dependencies = []
"#,
        )
        .unwrap();

        let inference = infer_initial_runtime(&root).unwrap();

        assert!(inference.components.contains("native-build"));
        assert!(inference.components.contains("linux-headers"));
        let evdev_source = "./vendor/robot_stack/pyproject.toml:project.optional-dependencies.full";
        assert!(inference.matches.iter().any(|matched| {
            matched.package == "evdev"
                && matched.component == "linux-headers"
                && matched.sources == vec![evdev_source.to_string()]
        }));
        assert!(inference.diagnostics.is_empty());

        cleanup(root);
    }

    #[test]
    fn inference_diagnoses_uninspectable_local_sources() {
        let root = temp_project("missing-local-source");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "local-native",
]

[tool.uv.sources]
local-native = { path = "vendor/missing" }
"#,
        )
        .unwrap();

        let inference = infer_initial_runtime(&root).unwrap();

        assert!(inference.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .summary
                .contains("could not inspect local source `./vendor/missing`")
        }));

        cleanup(root);
    }

    #[test]
    fn inference_uses_uv_lock_transitive_packages() {
        let root = temp_project("uv-lock-transitives");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = []
"#,
        )
        .unwrap();
        fs::write(
            root.join("uv.lock"),
            r#"version = 1
revision = 3

[[package]]
name = "evdev"
version = "1.9.3"
"#,
        )
        .unwrap();

        let inference = infer_initial_runtime(&root).unwrap();

        assert!(inference.components.contains("native-build"));
        assert!(inference.components.contains("linux-headers"));
        assert!(inference.matches.iter().any(|matched| {
            matched.package == "evdev"
                && matched.component == "linux-headers"
                && matched.sources == vec!["uv.lock".to_string()]
        }));

        cleanup(root);
    }

    #[test]
    fn uv_lock_package_scan_reads_package_names_only() {
        assert_eq!(
            uv_lock_package_names(
                r#"version = 1

[[package]]
name = "accelerate"
dependencies = [
    { name = "torch" },
]

[[package]]
name = "torch"
"#
            ),
            vec!["accelerate".to_string(), "torch".to_string()]
        );
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-inference-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
