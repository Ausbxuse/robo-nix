use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::manifest::{Manifest, ScriptDiscovery};

// Flat facts from project probing. Keep policy decisions in spec/pipeline, not here.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProbeResult {
    pub(super) env_name: Option<String>,
    pub(super) python_version: Option<ProbeValue>,
    pub(super) cuda_wheel_version: Option<ProbeValue>,
    pub(super) components: Vec<ProbeComponent>,
    pub(super) required_dirs: Vec<String>,
    pub(super) source_scripts: Vec<String>,
    pub(super) notes: Vec<String>,
    pub(super) suggestions: Vec<ProbeSuggestion>,
    pub(super) component_suggestions: Vec<ProbeComponentSuggestion>,
}

impl ProbeResult {
    fn add_component(&mut self, name: &str, note: &str) {
        self.components.push(ProbeComponent {
            name: name.to_string(),
            note: note.to_string(),
        });
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
pub(super) struct ProbeSuggestion {
    pub(super) kind: String,
    pub(super) path: String,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProbeComponentSuggestion {
    pub(super) name: String,
    pub(super) evidence: String,
    pub(super) reason: String,
}

pub(super) fn probe_project(target: &Path, manifest: &Manifest) -> ProbeResult {
    let mut probe = ProbeResult::default();
    let pyproject = target.join("pyproject.toml");
    if let Ok(text) = fs::read_to_string(&pyproject) {
        probe_pyproject_name(&text, &mut probe);
        probe_python_version(&text, &mut probe);
        probe_dependencies(&text, manifest, &mut probe);
    }
    probe_workspace(target, manifest, &mut probe);
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
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name") {
            if let Some(name) = crate::quoted_value(value) {
                probe.env_name = Some(name.to_string());
                return;
            }
        }
    }
}

fn probe_python_version(text: &str, probe: &mut ProbeResult) {
    if let Some(version) = crate::exact_python_requirement(text) {
        probe.python_version = Some(ProbeValue {
            value: version.to_string(),
            note: format!("python {version}: pyproject.toml requires-python"),
        });
        return;
    }

    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("requires-python") {
            if let Some(raw) = crate::quoted_value(value) {
                if let Some(version) = infer_python_version(raw) {
                    probe.python_version = Some(ProbeValue {
                        value: version.to_string(),
                        note: format!("python {version}: pyproject.toml requires-python"),
                    });
                    return;
                }
            }
        }
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
    let deps = text.to_ascii_lowercase();
    for rule in &manifest.runtime_inference.dependency_rules {
        if has_dep(&deps, &rule.dependencies) {
            for component in &rule.components {
                probe.add_component(component, &rule.note);
            }
        }
    }
}

fn probe_workspace(target: &Path, manifest: &Manifest, probe: &mut ProbeResult) {
    probe_workspace_directories(target, manifest, probe);
    probe_workspace_scripts(target, manifest, probe);
    probe_workspace_cuda_markers(target, probe);
}

fn probe_workspace_directories(
    target: &Path,
    manifest: &Manifest,
    probe: &mut ProbeResult,
) {
    for rule in &manifest.runtime_inference.workspace_directory_rules {
        if let Ok(entries) = fs::read_dir(target.join(&rule.root)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|ty| ty.is_dir()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let path = format!("{}/{}", rule.root, name);
                    probe.required_dirs.push(path);
                    if contains_any(&name.to_ascii_lowercase(), &rule.name_contains) {
                        for component in &rule.components {
                            probe.add_component(component, &rule.note);
                        }
                    }
                }
            }
        }
    }
}

fn probe_workspace_scripts(target: &Path, manifest: &Manifest, probe: &mut ProbeResult) {
    let discovery = &manifest.runtime_inference.script_discovery;
    for root in &discovery.roots {
        let root_path = target.join(root);
        if let Ok(entries) = fs::read_dir(&root_path) {
            for entry in entries.flatten() {
                if !entry.file_type().is_ok_and(|ty| ty.is_file()) {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if !is_discovered_script(&name, discovery) {
                    continue;
                }
                let relative = format!("{root}/{name}");
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if looks_like_daemon(&text, discovery) {
                        probe.notes.push(format!(
                            "skipped bootstrap {relative}: appears to start a long-running process"
                        ));
                    } else {
                        probe.source_scripts.push(relative);
                    }
                    probe_script_rules(&text, manifest, probe);
                    probe_script_paths(&text, discovery, probe);
                }
            }
        }
    }
}

fn probe_workspace_cuda_markers(target: &Path, probe: &mut ProbeResult) {
    let mut remaining = 2000usize;
    if let Some(evidence) = find_cuda_marker(target, target, 0, &mut remaining) {
        probe.component_suggestions.push(ProbeComponentSuggestion {
            name: "cuda-toolkit".to_string(),
            evidence,
            reason: "workspace contains CUDA extension markers".to_string(),
        });
    }
}

fn find_cuda_marker(root: &Path, path: &Path, depth: usize, remaining: &mut usize) -> Option<String> {
    if depth > 6 || *remaining == 0 {
        return None;
    }
    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        if *remaining == 0 {
            return None;
        }
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_probe_path(&name) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if let Some(evidence) = find_cuda_marker(root, &entry_path, depth + 1, remaining) {
                return Some(evidence);
            }
        } else if file_type.is_file() {
            *remaining -= 1;
            let relative = entry_path
                .strip_prefix(root)
                .unwrap_or(&entry_path)
                .display()
                .to_string();
            if matches!(
                entry_path.extension().and_then(|extension| extension.to_str()),
                Some("cu" | "cuh")
            ) {
                return Some(format!("{relative}: CUDA source file"));
            }
            if likely_cuda_build_file(&name) {
                if let Ok(text) = fs::read_to_string(&entry_path) {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("cudaextension")
                        || lower.contains("cuda_extension")
                        || lower.contains("cudatoolkit")
                        || lower.contains("nvcc")
                    {
                        return Some(format!("{relative}: CUDA build marker"));
                    }
                }
            }
        }
    }
    None
}

fn should_skip_probe_path(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".mypy_cache"
            | ".nox"
            | ".pytest_cache"
            | ".robo-nix"
            | ".tox"
            | ".venv"
            | "__pycache__"
            | "node_modules"
    )
}

fn likely_cuda_build_file(name: &str) -> bool {
    matches!(
        name,
        "pyproject.toml" | "setup.cfg" | "setup.py" | "CMakeLists.txt" | "Makefile" | "makefile"
    )
}

fn probe_script_rules(text: &str, manifest: &Manifest, probe: &mut ProbeResult) {
    let lower = text.to_ascii_lowercase();
    for rule in &manifest.runtime_inference.script_rules {
        if contains_any(&lower, &rule.text_contains) {
            for component in &rule.components {
                probe.add_component(component, &rule.note);
            }
        }
    }
}

fn probe_script_paths(text: &str, discovery: &ScriptDiscovery, probe: &mut ProbeResult) {
    let mut roots = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, rest)) = line.split_once('=') {
            if let Some(index) = rest.find(&discovery.path_root) {
                let path = rest[index..]
                    .trim_matches(|ch: char| {
                        ch == '"' || ch == '\'' || ch == '$' || ch == '{' || ch == '}'
                    })
                    .split(|ch: char| ch == '"' || ch == '\'' || ch.is_whitespace())
                    .next()
                    .unwrap_or("");
                if !path.is_empty() {
                    roots.insert(name.trim().to_string(), path.to_string());
                }
            }
        }
    }
    for line in text.lines() {
        if !line.contains(&discovery.checkout_function) {
            continue;
        }
        let mut tokens = line.split_whitespace().skip(1);
        let Some(root_token) = tokens.next() else {
            continue;
        };
        let var = root_token.trim_matches(|ch| ch == '"' || ch == '\'' || ch == '$');
        let Some(base) = roots.get(var) else {
            continue;
        };
        for token in tokens {
            let token = token.trim_matches(|ch| ch == '"' || ch == '\'' || ch == ';');
            if token.is_empty() {
                continue;
            }
            let path = format!("{base}/{token}");
            if token.contains('.') || token.contains('/') {
                probe.suggestions.push(ProbeSuggestion {
                    kind: "file".to_string(),
                    path,
                    reason: "bootstrap script referenced a local source checkout file".to_string(),
                });
            } else {
                probe.suggestions.push(ProbeSuggestion {
                    kind: "directory".to_string(),
                    path,
                    reason: "bootstrap script referenced a local source checkout directory"
                        .to_string(),
                });
            }
        }
    }
}

fn is_discovered_script(name: &str, discovery: &ScriptDiscovery) -> bool {
    discovery.names.iter().any(|item| item == name)
        || discovery.prefixes.iter().any(|prefix| name.starts_with(prefix))
}

fn looks_like_daemon(text: &str, discovery: &ScriptDiscovery) -> bool {
    discovery
        .daemon_text_contains
        .iter()
        .any(|pattern| text.contains(pattern))
}

fn has_dep(text: &str, names: &[String]) -> bool {
    names
        .iter()
        .any(|name| text.contains(&format!("\"{name}")) || text.contains(&format!("'{name}")))
}

fn contains_any(text: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| text.contains(&pattern.to_ascii_lowercase()))
}
