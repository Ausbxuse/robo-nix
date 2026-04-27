use crate::{Config, error, hint, ok, status};
use clap::Args;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

#[derive(Args)]
pub struct InitArgs {
    #[arg(help = "Project directory to initialize")]
    pub target: Option<PathBuf>,

    #[arg(long, help = "Run the guided initializer")]
    pub interactive: bool,

    #[arg(long, help = "List recommended starter profiles")]
    pub list_profiles: bool,

    #[arg(long, help = "List reusable runtime components")]
    pub list_components: bool,

    #[arg(long, help = "Print generated flake.nix instead of writing files")]
    pub stdout: bool,

    #[arg(long, help = "Overwrite generated flake.nix")]
    pub force: bool,

    #[arg(long, value_name = "NAME", help = "Environment name")]
    pub name: Option<String>,

    #[arg(long, value_name = "NAME", help = "Apply a recommended profile")]
    pub profile: Option<String>,

    #[arg(long = "with", value_name = "LIST", help = "Add comma-separated components")]
    pub with_components: Option<String>,

    #[arg(long, help = "Disable pyproject/workspace runtime probing")]
    pub no_probe: bool,

    #[arg(long, value_name = "TEXT", help = "Environment description")]
    pub description: Option<String>,

    #[arg(long, value_name = "PATH", default_value = ".", help = "Workspace root inside the project")]
    pub workspace_root: String,

    #[arg(long, value_name = "LIST", help = "Comma-separated component names")]
    pub components: Option<String>,

    #[arg(long, value_name = "VERSION", help = "Python version for uv")]
    pub python_version: Option<String>,

    #[arg(long, value_name = "LIST", help = "Comma-separated Nix systems")]
    pub systems: Option<String>,

    #[arg(long, value_name = "PATH", help = "Require a project-owned directory")]
    pub required_dir: Vec<String>,

    #[arg(long, value_name = "PATH", help = "Require a project-owned file")]
    pub required_file: Vec<String>,

    #[arg(long, value_name = "PATH", help = "Source a project-owned bootstrap script")]
    pub source_script: Vec<String>,

    #[arg(long, value_name = "NAME=VALUE", help = "Export a project runtime variable")]
    pub env: Vec<String>,

    #[arg(long, value_name = "URL", help = "robo-nix input URL to embed in flake.nix")]
    pub robo_nix_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    components: BTreeMap<String, Component>,
    profiles: BTreeMap<String, Profile>,
    runtime_inference: RuntimeInference,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Component {
    category: String,
    description: String,
    scaffold_directories: Vec<String>,
    supported_systems: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    description: String,
    components: Vec<String>,
    python_version: String,
    supported_systems: Vec<String>,
    workspace_root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInference {
    default_profile: String,
    dependency_rules: Vec<DependencyRule>,
    workspace_directory_rules: Vec<WorkspaceDirectoryRule>,
    script_discovery: ScriptDiscovery,
    script_rules: Vec<ScriptRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyRule {
    dependencies: Vec<String>,
    components: Vec<String>,
    note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDirectoryRule {
    root: String,
    name_contains: Vec<String>,
    components: Vec<String>,
    note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptDiscovery {
    roots: Vec<String>,
    names: Vec<String>,
    prefixes: Vec<String>,
    daemon_text_contains: Vec<String>,
    checkout_function: String,
    path_root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptRule {
    text_contains: Vec<String>,
    components: Vec<String>,
    note: String,
}

#[derive(Clone)]
struct ProjectSpec {
    profile_name: String,
    env_name: String,
    description: String,
    components: Vec<String>,
    python_version: String,
    supported_systems: Vec<String>,
    workspace_root: String,
    required_dirs: Vec<String>,
    required_files: Vec<String>,
    source_scripts: Vec<String>,
    env: Vec<String>,
    probe_notes: Vec<String>,
    component_provenance: Vec<ComponentProvenance>,
    suggestions: Vec<InferenceSuggestion>,
}

impl ProjectSpec {
    fn from_profile(name: &str, manifest: &Manifest) -> Result<Self, String> {
        let profile = manifest
            .profiles
            .get(name)
            .ok_or_else(|| format!("unknown profile: {name}"))?;
        Ok(Self {
            profile_name: name.to_string(),
            env_name: "project".to_string(),
            description: profile.description.clone(),
            components: profile.components.clone(),
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
        })
    }

    fn add_component(&mut self, component: &str, note: impl Into<String>) {
        let note = note.into();
        let source = inference_source(&note);
        self.add_component_with_source(component, source, note);
    }

    fn add_component_with_source(&mut self, component: &str, source: &str, reason: impl Into<String>) {
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

    fn add_required_dir(&mut self, path: &str) {
        push_unique(&mut self.required_dirs, path);
    }

    fn add_required_file(&mut self, path: &str) {
        push_unique(&mut self.required_files, path);
    }

    fn add_suggestion(&mut self, kind: &str, path: &str, reason: impl Into<String>) {
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
}

#[derive(Clone)]
struct ComponentProvenance {
    name: String,
    source: String,
    reason: String,
}

#[derive(Clone)]
struct InferenceSuggestion {
    kind: String,
    path: String,
    reason: String,
}

pub fn run(args: InitArgs, config: Config) -> ExitCode {
    let manifest = match load_manifest(config) {
        Ok(manifest) => manifest,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };

    if args.list_profiles {
        list_profiles(&manifest);
        return ExitCode::SUCCESS;
    }
    if args.list_components {
        list_components(&manifest);
        return ExitCode::SUCCESS;
    }

    let mut args = args;
    if args.interactive {
        if let Err(code) = interactive(&mut args, &manifest, config) {
            return code;
        }
    }

    let default_profile = args
        .profile
        .as_deref()
        .unwrap_or(&manifest.runtime_inference.default_profile);
    let mut spec = match ProjectSpec::from_profile(default_profile, &manifest) {
        Ok(spec) => spec,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };

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

    let target_dir = args.target.clone().unwrap_or_else(|| PathBuf::from("."));
    if spec.env_name == "project" {
        if let Some(name) = target_dir.file_name().and_then(|name| name.to_str()) {
            if !name.is_empty() && name != "." {
                spec.env_name = name.to_string();
            }
        }
    }
    if !args.no_probe {
        probe_project(&target_dir, &manifest, &mut spec);
    }
    dedupe_all(&mut spec);

    if let Err(message) = validate(&manifest, &spec) {
        error(config, &message);
        return ExitCode::from(1);
    }

    let source_url = args
        .robo_nix_url
        .clone()
        .or_else(|| env::var("ROBO_NIX_DEFAULT_SOURCE_URL").ok())
        .unwrap_or_else(|| "path:.".to_string());
    let flake = render_flake(&source_url);
    let project = render_project(&spec);

    if args.stdout {
        println!("{flake}");
        return ExitCode::SUCCESS;
    }

    status(config, "initializing runtime");
    match write_project(&manifest, &target_dir, &flake, &project, &spec, args.force, &source_url, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

pub fn cuda_doctor(config: Config) -> ExitCode {
    println!("cuda-doctor: starting");
    if env::consts::OS != "linux" {
        error(config, "CUDA validation is only supported on Linux hosts.");
        return ExitCode::from(1);
    }
    if Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        error(config, "missing nvidia-smi; this host does not appear to have NVIDIA drivers installed.");
        return ExitCode::from(1);
    }

    let cuda_root = env::var("CUDA_HOME")
        .or_else(|_| env::var("CUDA_PATH"))
        .unwrap_or_else(|_| "/usr/local/cuda".to_string());
    if !Path::new(&cuda_root).is_dir() {
        error(config, &format!("CUDA root not found at {cuda_root}"));
        hint(config, "set CUDA_HOME or CUDA_PATH if CUDA is installed elsewhere.");
        return ExitCode::from(1);
    }
    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            println!("cuda-doctor: nvidia-smi OK");
            println!("cuda-doctor: cuda_root={cuda_root}");
            println!("cuda-doctor: status=ok");
            ExitCode::SUCCESS
        }
        _ => {
            error(config, "nvidia-smi failed; GPU driver stack is not healthy.");
            ExitCode::from(1)
        }
    }
}

fn load_manifest(_config: Config) -> Result<Manifest, String> {
    let path = env::var("ROBO_NIX_COMPONENT_MANIFEST")
        .map_err(|_| "ROBO_NIX_COMPONENT_MANIFEST is not set.".to_string())?;
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read component manifest {path}: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("failed to parse component manifest: {err}"))
}

fn list_profiles(manifest: &Manifest) {
    for (name, profile) in &manifest.profiles {
        println!(
            "{:<18} {:<30} {}",
            name,
            profile.supported_systems.join(","),
            profile.description
        );
    }
}

fn list_components(manifest: &Manifest) {
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

fn profile_names(manifest: &Manifest) -> Vec<String> {
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

fn resolve_profile_selection(profiles: &[String], selection: &str) -> Option<String> {
    if let Ok(index) = selection.parse::<usize>() {
        return profiles.get(index.checked_sub(1)?).cloned();
    }
    profiles.iter().find(|profile| *profile == selection).cloned()
}

fn interactive(args: &mut InitArgs, manifest: &Manifest, config: Config) -> Result<(), ExitCode> {
    eprintln!("robo init");
    let advanced = ask("Advanced component selection?", "no", config)?;
    if matches!(advanced.as_str(), "yes" | "y") {
        list_profiles(manifest);
        let profile = ask("Profile", &manifest.runtime_inference.default_profile, config)?;
        args.profile = Some(profile);
    } else {
        let profiles = profile_names(manifest);
        eprintln!("Project setup:");
        for (index, profile) in profiles.iter().enumerate() {
            eprintln!("  {}. {}", index + 1, profile);
        }
        let setup = ask("Selection", "1", config)?;
        args.profile = Some(resolve_profile_selection(&profiles, &setup).ok_or_else(|| {
            error(config, &format!("unknown setup selection: {setup}"));
            ExitCode::from(1)
        })?);
    }

    if args.target.is_none() {
        let target = ask("Target directory", ".", config)?;
        args.target = Some(PathBuf::from(target));
    }
    if io::stdin().is_terminal() && args.name.is_none() {
        let env_name = ask("Environment name", "project", config)?;
        if !env_name.is_empty() {
            args.name = Some(env_name);
        }
    }
    let proceed = ask("Write runtime files?", "yes", config)?;
    if !matches!(proceed.as_str(), "yes" | "y") {
        return Err(ExitCode::SUCCESS);
    }
    Ok(())
}

fn ask(prompt: &str, default: &str, config: Config) -> Result<String, ExitCode> {
    if !io::stdin().is_terminal() {
        return Ok(default.to_string());
    }
    eprint!("{prompt} [{default}]: ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    if let Err(err) = io::stdin().read_line(&mut line) {
        error(config, &format!("failed to read input: {err}"));
        return Err(ExitCode::from(1));
    }
    let value = line.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn probe_project(target: &Path, manifest: &Manifest, spec: &mut ProjectSpec) {
    let pyproject = target.join("pyproject.toml");
    if let Ok(text) = fs::read_to_string(&pyproject) {
        probe_pyproject_name(&text, spec);
        probe_python_version(&text, spec);
        probe_dependencies(&text, manifest, spec);
    }
    probe_workspace(target, manifest, spec);
}

fn probe_pyproject_name(text: &str, spec: &mut ProjectSpec) {
    if spec.env_name != "project" {
        return;
    }
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name") {
            if let Some(name) = quoted_value(value) {
                spec.env_name = name;
                return;
            }
        }
    }
}

fn probe_python_version(text: &str, spec: &mut ProjectSpec) {
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("requires-python") {
            if let Some(raw) = quoted_value(value) {
                if let Some(version) = infer_python_version(&raw) {
                    spec.python_version = version.to_string();
                    spec.probe_notes.push(format!("python {version}: pyproject.toml requires-python"));
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
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
            .map_or(raw.len(), |end_delta| start + end_delta);
        let version = &raw[start..end];
        let parts = version.split('.').collect::<Vec<_>>();

        if matches!(parts.len(), 2 | 3) && parts.iter().all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit())) {
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

fn probe_dependencies(text: &str, manifest: &Manifest, spec: &mut ProjectSpec) {
    let deps = text.to_ascii_lowercase();
    for rule in &manifest.runtime_inference.dependency_rules {
        if has_dep(&deps, &rule.dependencies) {
            for component in &rule.components {
                spec.add_component(component, rule.note.clone());
            }
        }
    }
}

fn probe_workspace(target: &Path, manifest: &Manifest, spec: &mut ProjectSpec) {
    probe_workspace_directories(target, manifest, spec);
    probe_workspace_scripts(target, manifest, spec);
}

fn probe_workspace_directories(target: &Path, manifest: &Manifest, spec: &mut ProjectSpec) {
    for rule in &manifest.runtime_inference.workspace_directory_rules {
        if let Ok(entries) = fs::read_dir(target.join(&rule.root)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|ty| ty.is_dir()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let path = format!("{}/{}", rule.root, name);
                    spec.add_required_dir(&path);
                    if contains_any(&name.to_ascii_lowercase(), &rule.name_contains) {
                        for component in &rule.components {
                            spec.add_component(component, rule.note.clone());
                        }
                    }
                }
            }
        }
    }
}

fn probe_workspace_scripts(target: &Path, manifest: &Manifest, spec: &mut ProjectSpec) {
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
                        spec.probe_notes.push(format!(
                            "skipped bootstrap {relative}: appears to start a long-running process"
                        ));
                    } else {
                        push_unique(&mut spec.source_scripts, &relative);
                    }
                    probe_script_rules(&text, manifest, spec);
                    probe_script_paths(&text, discovery, spec);
                }
            }
        }
    }
}

fn probe_script_rules(text: &str, manifest: &Manifest, spec: &mut ProjectSpec) {
    let lower = text.to_ascii_lowercase();
    for rule in &manifest.runtime_inference.script_rules {
        if contains_any(&lower, &rule.text_contains) {
            for component in &rule.components {
                spec.add_component(component, rule.note.clone());
            }
        }
    }
}

fn probe_script_paths(text: &str, discovery: &ScriptDiscovery, spec: &mut ProjectSpec) {
    let mut roots = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, rest)) = line.split_once('=') {
            if let Some(index) = rest.find(&discovery.path_root) {
                let path = rest[index..]
                    .trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '$' || ch == '{' || ch == '}')
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
                spec.add_suggestion(
                    "file",
                    &path,
                    "bootstrap script referenced a vendor checkout file",
                );
            } else {
                spec.add_suggestion(
                    "directory",
                    &path,
                    "bootstrap script referenced a vendor checkout directory",
                );
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

fn validate(manifest: &Manifest, spec: &ProjectSpec) -> Result<(), String> {
    if spec.supported_systems.is_empty() {
        return Err("expected at least one value for systems".to_string());
    }
    for component in &spec.components {
        if !manifest.components.contains_key(component) {
            return Err(format!("unknown component: {component}"));
        }
    }
    for rule in &manifest.runtime_inference.dependency_rules {
        for component in &rule.components {
            if !manifest.components.contains_key(component) {
                return Err(format!(
                    "runtime inference rule references unknown component: {component}"
                ));
            }
        }
    }
    Ok(())
}

fn write_project(
    manifest: &Manifest,
    target: &Path,
    flake: &str,
    project: &str,
    spec: &ProjectSpec,
    force: bool,
    source_url: &str,
    config: Config,
) -> Result<(), ExitCode> {
    fs::create_dir_all(target).map_err(|err| {
        error(config, &format!("failed to create {}: {err}", target.display()));
        ExitCode::from(1)
    })?;

    let flake_path = target.join("flake.nix");
    if flake_path.exists() && !force {
        match fs::read_to_string(&flake_path) {
            Ok(text) if text.contains("mkProjectFlakeFromManifest") => {}
            _ => {
                error(config, &format!("{} does not look generated by robo-nix", flake_path.display()));
                hint(config, "rerun with --force only if you want robo to replace it");
                return Err(ExitCode::from(1));
            }
        }
    }

    write_file(&flake_path, flake, config)?;
    write_file(&target.join("robo.nix"), project, config)?;
    write_file(&target.join(".python-version"), &spec.python_version, config)?;
    let pyproject = target.join("pyproject.toml");
    let pyproject_status = if pyproject.exists() {
        "kept"
    } else {
        write_file(&pyproject, &render_pyproject(spec), config)?;
        "wrote"
    };

    for component in &spec.components {
        let Some(component) = manifest.components.get(component) else {
            continue;
        };
        for dir in &component.scaffold_directories {
            fs::create_dir_all(target.join(dir)).map_err(|err| {
                error(config, &format!("failed to create {}: {err}", target.join(dir).display()));
                ExitCode::from(1)
            })?;
        }
    }

    register_git(target);
    print_summary(target, spec, pyproject_status, source_url, config);
    Ok(())
}

fn write_file(path: &Path, text: &str, config: Config) -> Result<(), ExitCode> {
    fs::write(path, format!("{}\n", text.trim_end())).map_err(|err| {
        error(config, &format!("failed to write {}: {err}", path.display()));
        ExitCode::from(1)
    })
}

fn register_git(target: &Path) {
    let inside = Command::new("git")
        .arg("-C")
        .arg(target)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if inside {
        let _ = Command::new("git")
            .arg("-C")
            .arg(target)
            .args(["add", "--intent-to-add", "--"])
            .args(["flake.nix", "robo.nix", ".python-version", "pyproject.toml"])
            .status();
    }
}

fn print_summary(target: &Path, spec: &ProjectSpec, pyproject_status: &str, source_url: &str, config: Config) {
    eprintln!("{} init", crate::label(config, "robo", crate::LabelKind::Status));
    eprintln!("  target: {}", target.display());
    eprintln!("  env:    {}", spec.env_name);
    eprintln!("  source: {source_url}");
    if !spec.probe_notes.is_empty() {
        eprintln!();
        eprintln!("Detected:");
        for note in &spec.probe_notes {
            eprintln!("  + {note}");
        }
    }
    if !spec.suggestions.is_empty() {
        eprintln!();
        eprintln!("Suggestions:");
        for item in &spec.suggestions {
            eprintln!("  ? {} {}: {}", item.kind, item.path, item.reason);
        }
    }
    eprintln!();
    ok(config, "Generated:");
    eprintln!("  wrote {}", target.join("flake.nix").display());
    eprintln!("  wrote {}", target.join("robo.nix").display());
    eprintln!("  wrote {}", target.join(".python-version").display());
    eprintln!("  {pyproject_status} {}", target.join("pyproject.toml").display());
    eprintln!();
    eprintln!("{} Next steps:", crate::label(config, "robo", crate::LabelKind::Status));
    eprintln!("  cd {}", target.display());
    if source_url.starts_with("path:") {
        eprintln!("  nix flake lock --update-input robo-nix  # after local robo-nix source edits");
    }
    eprintln!("  robo doctor");
    eprintln!("  robo sync --group dev");
    eprintln!("  robo run pytest tests");
}

fn render_flake(source_url: &str) -> String {
    format!(
        r#"{{
  inputs.robo-nix.url = "{}";

  # NOTE: generated plumbing. Most users should edit robo.nix,
  # pyproject.toml, and .python-version instead of this file.
  outputs = {{robo-nix, ...}}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}}"#,
        escape_nix(source_url)
    )
}

fn render_project(spec: &ProjectSpec) -> String {
    let mut text = format!(
        r#"{{
  schemaVersion = 1;
  envName = "{}";
  description = "{}";
  components = [
{}
  ];
  pythonVersion = "{}";
  supportedSystems = [
{}
  ];
  workspaceRoot = "{}";"#,
        escape_nix(&spec.env_name),
        escape_nix(&spec.description),
        render_list(&spec.components),
        escape_nix(&spec.python_version),
        render_list(&spec.supported_systems),
        escape_nix(&spec.workspace_root)
    );
    if !spec.required_dirs.is_empty() {
        text.push_str(&format!("\n\n  requiredDirectories = [\n{}\n  ];", render_list(&spec.required_dirs)));
    }
    if !spec.required_files.is_empty() {
        text.push_str(&format!("\n\n  requiredFiles = [\n{}\n  ];", render_list(&spec.required_files)));
    }
    if !spec.env.is_empty() {
        text.push_str("\n\n  shellInit = ''\n");
        for item in &spec.env {
            if let Some((name, value)) = item.split_once('=') {
                text.push_str(&format!("    export {}=\"{}\"\n", name, value));
            }
        }
        text.push_str("  '';");
    }
    if !spec.source_scripts.is_empty() {
        text.push_str("\n\n  bootstrap = ''\n");
        for script in &spec.source_scripts {
            text.push_str(&format!("    . \"$WORKSPACE_ROOT/{}\"\n", script));
        }
        text.push_str("  '';");
    }
    {
        text.push_str("\n\n  provenance = {\n");
        text.push_str("    generatedBy = \"robo init\";\n");
        text.push_str(&format!("    profile = \"{}\";\n", escape_nix(&spec.profile_name)));
        if !spec.component_provenance.is_empty() {
            text.push_str("    componentReasons = [\n");
            for item in &spec.component_provenance {
                text.push_str("      {\n");
                text.push_str(&format!("        name = \"{}\";\n", escape_nix(&item.name)));
                text.push_str(&format!("        source = \"{}\";\n", escape_nix(&item.source)));
                text.push_str(&format!("        reason = \"{}\";\n", escape_nix(&item.reason)));
                text.push_str("      }\n");
            }
            text.push_str("    ];\n");
        }
        if !spec.probe_notes.is_empty() {
            text.push_str("    inferred = [\n");
            for note in &spec.probe_notes {
                text.push_str(&format!("      \"{}\"\n", escape_nix(note)));
            }
            text.push_str("    ];\n");
        }
        if !spec.suggestions.is_empty() {
            text.push_str("    suggestions = [\n");
            for item in &spec.suggestions {
                text.push_str("      {\n");
                text.push_str(&format!("        kind = \"{}\";\n", escape_nix(&item.kind)));
                text.push_str(&format!("        path = \"{}\";\n", escape_nix(&item.path)));
                text.push_str(&format!("        reason = \"{}\";\n", escape_nix(&item.reason)));
                text.push_str("      }\n");
            }
            text.push_str("    ];\n");
        }
        text.push_str("  };");
    }
    text.push_str("\n}");
    text
}

fn render_pyproject(spec: &ProjectSpec) -> String {
    format!(
        r#"[project]
name = "{}"
version = "0.1.0"
requires-python = ">={}"
dependencies = []
"#,
        escape_toml(&spec.env_name),
        escape_toml(&spec.python_version)
    )
}

fn render_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("    \"{}\"", escape_nix(item)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn has_dep(text: &str, names: &[String]) -> bool {
    names.iter().any(|name| text.contains(&format!("\"{name}")) || text.contains(&format!("'{name}")))
}

fn contains_any(text: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| text.contains(&pattern.to_ascii_lowercase()))
}

fn quoted_value(text: &str) -> Option<String> {
    let start = text.find('"')?;
    let end = text[start + 1..].find('"')?;
    Some(text[start + 1..start + 1 + end].to_string())
}

fn push_unique(items: &mut Vec<String>, value: &str) {
    if !items.iter().any(|item| item == value) {
        items.push(value.to_string());
    }
}

fn dedupe_all(spec: &mut ProjectSpec) {
    spec.components = dedupe(spec.components.clone());
    spec.supported_systems = dedupe(spec.supported_systems.clone());
    spec.required_dirs = dedupe(spec.required_dirs.clone());
    spec.required_files = dedupe(spec.required_files.clone());
    spec.source_scripts = dedupe(spec.source_scripts.clone());
    spec.env = dedupe(spec.env.clone());
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

fn escape_nix(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_toml(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
