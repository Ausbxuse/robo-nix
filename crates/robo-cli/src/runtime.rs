use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
#[cfg(all(target_os = "linux", not(target_env = "musl")))]
use std::ffi::{CString, c_char, c_int, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIBCUDA: &str = "libcuda.so.1";
const LIBCUDA_NAMES: &[&str] = &[LIBCUDA, "libcuda.so"];
const NVML_NAMES: &[&str] = &["libnvidia-ml.so.1", "libnvidia-ml.so"];
const KNOWN_HOST_LIBCUDA_DIRS: &[&str] = &[
    "/run/opengl-driver/lib",
    "/usr/lib64/nvidia",
    "/usr/lib/x86_64-linux-gnu",
    "/usr/lib/wsl/lib",
];
const KNOWN_HOST_EGL_VENDOR_DIRS: &[&str] = &[
    "/run/opengl-driver/share/glvnd/egl_vendor.d",
    "/usr/share/glvnd/egl_vendor.d",
    "/etc/glvnd/egl_vendor.d",
];
const KNOWN_HOST_VULKAN_ICD_DIRS: &[&str] = &[
    "/run/opengl-driver/share/vulkan/icd.d",
    "/usr/share/vulkan/icd.d",
    "/etc/vulkan/icd.d",
];

pub(crate) struct ProjectRuntime {
    pub(crate) schema_version: Option<String>,
    pub(crate) env_name: String,
    pub(crate) python_version: String,
    pub(crate) cuda_wheel_version: Option<String>,
    pub(crate) components: Vec<String>,
    pub(crate) suggestions: Vec<RuntimeSuggestion>,
}

#[derive(Clone)]
pub(crate) struct RuntimeSuggestion {
    pub(crate) kind: String,
    pub(crate) path: String,
    pub(crate) reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeWhy {
    pub(crate) env_name: String,
    pub(crate) python_version: String,
    pub(crate) profile: Option<String>,
    pub(crate) requirements: Vec<WhyEntry>,
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
    #[serde(default)]
    components: BTreeMap<String, RuntimeComponent>,
    profiles: BTreeMap<String, RuntimeProfile>,
    #[serde(default)]
    runtime_inference: RuntimeInference,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeComponent {
    #[serde(default)]
    provides: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProfile {
    components: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInference {
    #[serde(default)]
    dependency_rules: Vec<DependencyRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyRule {
    dependencies: Vec<String>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    components: Vec<String>,
    note: String,
}

pub(crate) struct ExpectedComponent {
    pub(crate) name: String,
    pub(crate) reason: String,
}

struct ExpectedRequirement {
    name: String,
    source: String,
    reason: String,
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

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifest {
    schema_version: Option<String>,
    env_name: Option<String>,
    profile: Option<String>,
    python_version: Option<String>,
    cuda_wheel_version: Option<String>,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    requirements: Vec<ProjectManifestRequirement>,
    #[serde(default)]
    required_directories: Vec<String>,
    #[serde(default)]
    required_files: Vec<String>,
    provenance: ProjectManifestProvenance,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifestProvenance {
    profile: Option<String>,
    inferred: Vec<String>,
    component_reasons: Vec<ProjectManifestComponentReason>,
    source_scripts: Vec<String>,
    suggestions: Vec<ProjectManifestSuggestion>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifestRequirement {
    id: String,
    #[serde(default = "default_inference_source")]
    source: String,
    #[serde(default = "default_requirement_reason")]
    reason: String,
    #[serde(default)]
    evidence: Option<String>,
    #[serde(default = "default_requirement_severity")]
    severity: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifestComponentReason {
    name: String,
    #[serde(default = "default_inference_source")]
    source: String,
    #[serde(default = "default_component_reason")]
    reason: String,
}

#[derive(Deserialize)]
struct ProjectManifestSuggestion {
    #[serde(default = "default_suggestion_kind")]
    kind: String,
    path: String,
    #[serde(default = "default_suggestion_reason")]
    reason: String,
}

pub(crate) fn read_project_runtime() -> ProjectRuntime {
    let manifest = read_project_manifest().unwrap_or_default();
    ProjectRuntime {
        schema_version: manifest.schema_version,
        env_name: manifest.env_name.unwrap_or_else(|| "project".to_string()),
        python_version: manifest
            .python_version
            .unwrap_or_else(|| "unknown".to_string()),
        cuda_wheel_version: manifest.cuda_wheel_version,
        components: manifest.components,
        suggestions: manifest
            .provenance
            .suggestions
            .into_iter()
            .map(|suggestion| RuntimeSuggestion {
                kind: suggestion.kind,
                path: suggestion.path,
                reason: suggestion.reason,
            })
            .collect(),
    }
}

pub(crate) fn infer_cuda_wheel_version_from_uv_lock() -> Option<String> {
    let lock = fs::read_to_string("uv.lock").ok()?;
    infer_cuda_wheel_version_from_uv_lock_text(&lock)
}

pub(crate) fn infer_cuda_wheel_version_from_uv_lock_text(text: &str) -> Option<String> {
    let mut package_name: Option<String> = None;
    let mut best: Option<(u32, u32)> = None;

    for line in text.lines() {
        let line = line.trim();

        if line.starts_with("[[package]]") {
            package_name = None;
            continue;
        }

        if let Some(value) = line.strip_prefix("name = ") {
            if is_cuda_package_name(&extract_quoted(value).unwrap_or("")) {
                package_name = extract_quoted(value).map(ToOwned::to_owned);
            } else {
                package_name = None;
            }
            continue;
        }

        if line.starts_with("version = ")
            && let Some(name) = package_name.as_deref()
            && is_cuda_package_name(name)
            && let Some(raw) = extract_quoted(line)
            && let Some(version) = parse_major_minor(raw)
            && best.is_none_or(|current| version > current)
        {
            best = Some(version);
        }
    }

    best.map(|(major, minor)| format!("{major}.{minor}"))
}

pub(crate) fn cuda_root_from_env() -> Option<String> {
    env::var("ROBO_NIX_CUDA_ROOT")
        .ok()
        .filter(|root| Path::new(root).is_dir())
        .or_else(|| {
            env::var("CUDA_HOME")
                .ok()
                .filter(|root| Path::new(root).is_dir())
        })
        .or_else(|| {
            env::var("CUDA_PATH")
                .ok()
                .filter(|root| Path::new(root).is_dir())
        })
}

pub(crate) fn cuda_version_from_root() -> Option<String> {
    let root = cuda_root_from_env()?;
    let root = Path::new(&root);
    read_cuda_version_file(root)
        .or_else(|| parse_cuda_version_from_path(root))
        .or_else(|| read_cuda_version_with_nvcc(root))
}

fn read_cuda_version_file(root: &Path) -> Option<String> {
    let path = root.join("version.txt");
    if !path.is_file() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(cuda_major_minor_version)
}

fn parse_cuda_version_from_path(root: &Path) -> Option<String> {
    let file_name = root.file_name()?.to_str()?;
    file_name.split('-').find_map(cuda_major_minor_version)
}

fn read_cuda_version_with_nvcc(root: &Path) -> Option<String> {
    let nvcc = root.join("bin").join("nvcc");
    if !nvcc.is_file() {
        return None;
    }
    let output = Command::new(nvcc).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let text2 = String::from_utf8_lossy(&output.stderr);
    find_cuda_release_version(&text).or_else(|| find_cuda_release_version(&text2))
}

fn find_cuda_release_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(index) = line.find("release ") {
            let rest = &line[index + "release ".len()..];
            if let Some(version) = cuda_major_minor_version(rest) {
                return Some(version);
            }
        }
    }
    None
}

pub(crate) fn cuda_release_version_from_text(text: &str) -> Option<String> {
    find_cuda_release_version(text)
}

pub(crate) fn host_cuda_driver_version() -> Option<String> {
    host_cuda_driver_version_from_nvml().or_else(host_cuda_driver_version_from_nvidia_smi)
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
fn host_cuda_driver_version_from_nvml() -> Option<String> {
    type NvmlInit = unsafe extern "C" fn() -> c_int;
    type NvmlShutdown = unsafe extern "C" fn() -> c_int;
    type NvmlSystemGetCudaDriverVersion = unsafe extern "C" fn(*mut c_int) -> c_int;

    let library = open_nvml_library()?;
    let init = unsafe {
        library
            .symbol::<NvmlInit>(b"nvmlInit_v2\0")
            .or_else(|| library.symbol::<NvmlInit>(b"nvmlInit\0"))
    }?;
    let shutdown = unsafe { library.symbol::<NvmlShutdown>(b"nvmlShutdown\0") }?;
    let get_version = unsafe {
        library
            .symbol::<NvmlSystemGetCudaDriverVersion>(b"nvmlSystemGetCudaDriverVersion_v2\0")
            .or_else(|| {
                library.symbol::<NvmlSystemGetCudaDriverVersion>(
                    b"nvmlSystemGetCudaDriverVersion\0",
                )
            })
    }?;

    if unsafe { init() } != 0 {
        return None;
    }

    let mut version = 0;
    let result = unsafe { get_version(&mut version) };
    let _ = unsafe { shutdown() };
    if result != 0 {
        return None;
    }

    cuda_driver_api_version(version)
}

#[cfg(any(not(target_os = "linux"), target_env = "musl"))]
fn host_cuda_driver_version_from_nvml() -> Option<String> {
    None
}

fn host_cuda_driver_version_from_nvidia_smi() -> Option<String> {
    let output = Command::new("nvidia-smi").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    find_nvidia_smi_cuda_version(&stdout).or_else(|| find_nvidia_smi_cuda_version(&stderr))
}

pub(crate) fn cuda_version_less_than(actual: &str, expected: &str) -> Option<bool> {
    Some(parse_major_minor(actual)? < parse_major_minor(expected)?)
}

pub(crate) fn find_host_libcuda() -> Option<String> {
    find_libcuda_from_env()
        .or_else(find_libcuda_in_library_path)
        .or_else(find_libcuda_with_ldconfig)
        .or_else(find_libcuda_in_known_host_locations)
}

pub(crate) fn find_host_nvidia_egl_vendor_file() -> Option<String> {
    find_first_nvidia_json(KNOWN_HOST_EGL_VENDOR_DIRS)
}

pub(crate) fn find_host_nvidia_vulkan_icd_file() -> Option<String> {
    find_first_nvidia_json(KNOWN_HOST_VULKAN_ICD_DIRS)
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
struct DynamicLibrary(*mut c_void);

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
impl DynamicLibrary {
    fn open(path: &str) -> Option<Self> {
        let path = CString::new(path).ok()?;
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
        (!handle.is_null()).then(|| Self(handle))
    }

    unsafe fn symbol<T>(&self, name: &[u8]) -> Option<T> {
        let symbol = unsafe { dlsym(self.0, name.as_ptr().cast()) };
        (!symbol.is_null()).then(|| unsafe { std::mem::transmute_copy(&symbol) })
    }
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        let _ = unsafe { dlclose(self.0) };
    }
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
const RTLD_LAZY: c_int = 1;

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
fn open_nvml_library() -> Option<DynamicLibrary> {
    for name in NVML_NAMES {
        if let Some(library) = DynamicLibrary::open(name) {
            return Some(library);
        }
    }

    for dir in KNOWN_HOST_LIBCUDA_DIRS {
        for name in NVML_NAMES {
            let path = Path::new(dir).join(name);
            if let Some(library) = DynamicLibrary::open(&path.to_string_lossy()) {
                return Some(library);
            }
        }
    }

    None
}

fn find_libcuda_with_ldconfig() -> Option<String> {
    let output = Command::new("ldconfig").arg("-p").output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let line = line.trim();
            if !LIBCUDA_NAMES.iter().any(|name| line.starts_with(name)) {
                return None;
            }
            line.rsplit_once(" => ")
                .map(|(_, path)| path.trim().to_string())
                .filter(|path| Path::new(path).is_file())
        })
}

fn find_libcuda_in_library_path() -> Option<String> {
    env::var_os("LD_LIBRARY_PATH").and_then(|paths| {
        env::split_paths(&paths)
            .find_map(|dir| find_libcuda_in_dir(&dir))
            .map(|path| path.display().to_string())
    })
}

fn find_libcuda_in_known_host_locations() -> Option<String> {
    KNOWN_HOST_LIBCUDA_DIRS
        .iter()
        .map(Path::new)
        .find_map(find_libcuda_in_dir)
        .map(|path| path.display().to_string())
}

fn find_libcuda_from_env() -> Option<String> {
    let path = env::var("ROBO_NIX_LIBCUDA_PATH").ok()?;
    let path = Path::new(&path);
    if path.is_file() {
        Some(path.display().to_string())
    } else if path.is_dir() {
        find_libcuda_in_dir(path).map(|libcuda| libcuda.display().to_string())
    } else {
        None
    }
}

fn find_libcuda_in_dir(dir: &Path) -> Option<PathBuf> {
    LIBCUDA_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

// This is intentionally narrow: only trust NVIDIA-named manifests from standard
// GLVND/Vulkan manifest directories that the host graphics stack already owns.
fn find_first_nvidia_json(dirs: &[&str]) -> Option<String> {
    dirs.iter()
        .map(Path::new)
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| fs::read_dir(dir).ok().into_iter().flatten())
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    let name = name.to_ascii_lowercase();
                    name.contains("nvidia") && name.ends_with(".json")
                })
        })
        .map(|path| path.display().to_string())
        .next()
}

fn cuda_driver_api_version(version: i32) -> Option<String> {
    (version > 0).then(|| format!("{}.{}", version / 1000, (version % 1000) / 10))
}

fn cuda_major_minor_version(text: &str) -> Option<String> {
    parse_major_minor(text).map(|(major, minor)| format!("{major}.{minor}"))
}

fn find_nvidia_smi_cuda_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some((_, rest)) = line.split_once("CUDA Version:") {
            return cuda_major_minor_version(rest);
        }
    }
    None
}

fn parse_major_minor(text: &str) -> Option<(u32, u32)> {
    let mut parts = text
        .split(|ch: char| ch == '.' || ch == ' ' || ch == '_' || ch == '-')
        .filter(|part| !part.is_empty())
        .map(|part| part.trim_matches(|c: char| !c.is_ascii_digit() && c != '.'));

    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

pub(crate) fn build_runtime_why(runtime: &ProjectRuntime) -> RuntimeWhy {
    let manifest = read_project_manifest().unwrap_or_default();
    let runtime_manifest = read_runtime_manifest();
    let profile = manifest.profile.or(manifest.provenance.profile);
    let provenance = ProjectProvenance {
        profile: profile.clone(),
        inferred: manifest.provenance.inferred,
        component_reasons: component_reasons(manifest.provenance.component_reasons),
        required_dirs: manifest.required_directories,
        required_files: manifest.required_files,
        bootstrap_scripts: manifest.provenance.source_scripts,
    };
    let profile_components = provenance
        .profile
        .as_deref()
        .and_then(|profile| profile_components(runtime_manifest.as_ref()?, profile))
        .unwrap_or_default();
    let mut pyproject_component_reasons = HashMap::new();
    let mut requirements: Vec<WhyEntry> = manifest
        .requirements
        .iter()
        .map(explain_requirement)
        .collect();
    let pyproject = fs::read_to_string("pyproject.toml").ok();
    if let Some(runtime_manifest) = &runtime_manifest {
        if let Some(pyproject) = pyproject.as_deref() {
            for component in expected_components_from_pyproject_with_manifest(
                runtime_manifest,
                pyproject,
            ) {
                pyproject_component_reasons.insert(component.name, component.reason);
            }
            requirements.extend(
                expected_requirements_from_pyproject_with_manifest(runtime_manifest, pyproject)
                    .into_iter()
                    .map(explain_expected_requirement),
            );
        }
        requirements.extend(component_requirements(
            runtime,
            &provenance,
            &profile_components,
            &pyproject_component_reasons,
            runtime_manifest,
        ));
    }

    RuntimeWhy {
        env_name: runtime.env_name.clone(),
        python_version: runtime.python_version.clone(),
        profile,
        components: runtime
            .components
            .iter()
            .map(|component| {
                explain_component(
                    component,
                    &provenance,
                    &profile_components,
                    &pyproject_component_reasons,
                )
            })
            .collect(),
        requirements: dedupe_why_entries(requirements),
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
                source: "manual config".to_string(),
                reason: "listed in provenance.sourceScripts in robo.nix".to_string(),
                remove_hint: format!("remove `{path}` from the bootstrap block in robo.nix"),
                remediation_hint: format!(
                    "create `{path}` or remove it if this project does not need that bootstrap step"
                ),
            })
            .collect(),
        suggestions: runtime
            .suggestions
            .iter()
            .map(|suggestion| WhyEntry {
                name: suggestion.path.clone(),
                source: "provenance.suggestions".to_string(),
                reason: suggestion.reason.clone(),
                remove_hint: "delete this entry from provenance.suggestions in robo.nix".to_string(),
                remediation_hint: suggestion_remediation_hint(suggestion),
            })
            .collect(),
    }
}

fn suggestion_remediation_hint(suggestion: &RuntimeSuggestion) -> String {
    if suggestion.kind == "bootstrap" {
        "delete this stale bootstrap suggestion; bootstrap blocks should come from explicit project policy".to_string()
    } else {
        format!(
            "promote `{}` to requiredFiles or requiredDirectories only if bootstrap truly depends on it",
            suggestion.path
        )
    }
}

fn default_suggestion_kind() -> String {
    "path".to_string()
}

fn default_suggestion_reason() -> String {
    "optional low-confidence source/runtime inference".to_string()
}

pub(crate) fn expected_components_from_pyproject(text: &str) -> Vec<ExpectedComponent> {
    let Some(manifest) = read_runtime_manifest() else {
        return Vec::new();
    };
    expected_components_from_pyproject_with_manifest(&manifest, text)
}

fn expected_components_from_pyproject_with_manifest(
    manifest: &RuntimeManifest,
    text: &str,
) -> Vec<ExpectedComponent> {
    let dependencies = crate::pyproject::dependency_names(text);
    let mut seen = HashSet::new();
    let mut expected = Vec::new();

    for rule in &manifest.runtime_inference.dependency_rules {
        if !rule
            .dependencies
            .iter()
            .any(|name| crate::pyproject::has_dependency_name(&dependencies, [name.as_str()]))
        {
            continue;
        }
        let mut components = rule.components.clone();
        for requirement in &rule.requires {
            for provider in providers_for(&manifest, requirement) {
                if !components.iter().any(|item| item == &provider) {
                    components.push(provider);
                }
            }
        }
        for component in components {
            if seen.insert(component.clone()) {
                expected.push(ExpectedComponent {
                    name: component,
                    reason: rule.note.clone(),
                });
            }
        }
    }

    expected
}

fn expected_requirements_from_pyproject_with_manifest(
    manifest: &RuntimeManifest,
    text: &str,
) -> Vec<ExpectedRequirement> {
    let dependencies = crate::pyproject::dependency_names(text);
    let mut seen = HashSet::new();
    let mut expected = Vec::new();

    for rule in &manifest.runtime_inference.dependency_rules {
        if !rule
            .dependencies
            .iter()
            .any(|name| crate::pyproject::has_dependency_name(&dependencies, [name.as_str()]))
        {
            continue;
        }
        for requirement in &rule.requires {
            if seen.insert(requirement.clone()) {
                expected.push(ExpectedRequirement {
                    name: requirement.clone(),
                    source: "pyproject inference".to_string(),
                    reason: rule.note.clone(),
                });
            }
        }
    }

    expected
}

fn providers_for(manifest: &RuntimeManifest, requirement: &str) -> Vec<String> {
    if requirement.starts_with("host.") {
        return Vec::new();
    }
    manifest
        .components
        .iter()
        .filter(|(_, component)| component.provides.iter().any(|item| item == requirement))
        .map(|(name, _)| name.clone())
        .collect()
}

fn explain_requirement(requirement: &ProjectManifestRequirement) -> WhyEntry {
    let mut reason = requirement.reason.clone();
    if let Some(evidence) = &requirement.evidence {
        reason = format!("{reason}; evidence: {evidence}");
    }
    if requirement.severity != "required" {
        reason = format!("{reason}; severity: {}", requirement.severity);
    }
    WhyEntry {
        name: requirement.id.clone(),
        source: requirement.source.clone(),
        reason,
        remove_hint: format!(
            "remove requirement `{}` from requirements in robo.nix if the inference is wrong",
            requirement.id
        ),
        remediation_hint: if requirement.id.starts_with("host.") {
            format!(
                "fix host/container visibility for `{}`; Nix components cannot provide host-owned capabilities",
                requirement.id
            )
        } else {
            format!(
                "keep a component that provides `{}` or rerun `robo init . --force` after dependency changes",
                requirement.id
            )
        },
    }
}

fn explain_expected_requirement(requirement: ExpectedRequirement) -> WhyEntry {
    WhyEntry {
        remove_hint: format!(
            "remove the dependency that inferred `{}` or edit `components` in robo.nix if the provider is unnecessary",
            requirement.name
        ),
        remediation_hint: if requirement.name.starts_with("host.") {
            format!(
                "fix host/container visibility for `{}`; Nix components cannot provide host-owned capabilities",
                requirement.name
            )
        } else {
            format!(
                "keep a component that provides `{}` or rerun `robo init . --force` after dependency changes",
                requirement.name
            )
        },
        name: requirement.name,
        source: requirement.source,
        reason: requirement.reason,
    }
}

fn explain_component(
    component: &str,
    provenance: &ProjectProvenance,
    profile_components: &[String],
    pyproject_component_reasons: &HashMap<String, String>,
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
            remediation_hint: "run `robo check --why` after edits to confirm the runtime contract still matches the project".to_string(),
        }
    } else if let Some(reason) = pyproject_component_reasons.get(component) {
        WhyEntry {
            name: component.to_string(),
            source: "pyproject inference".to_string(),
            reason: reason.clone(),
            remove_hint: format!(
                "remove `{component}` from `components` in robo.nix if the inference is wrong"
            ),
            remediation_hint: "run `robo check --why` after edits to confirm the runtime contract still matches the project".to_string(),
        }
    } else if !provenance.inferred.is_empty() {
        WhyEntry {
            name: component.to_string(),
            source: "inference".to_string(),
            reason: first_inference(provenance)
                .unwrap_or_else(|| "inferred from project metadata".to_string()),
            remove_hint: format!(
                "remove `{component}` from `components` in robo.nix if the inference is wrong"
            ),
            remediation_hint: "run `robo check --why` after edits to confirm the inferred runtime still matches the project".to_string(),
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

fn component_requirements(
    runtime: &ProjectRuntime,
    provenance: &ProjectProvenance,
    profile_components: &[String],
    pyproject_component_reasons: &HashMap<String, String>,
    manifest: &RuntimeManifest,
) -> Vec<WhyEntry> {
    let mut entries = Vec::new();
    for component in &runtime.components {
        let Some(metadata) = manifest.components.get(component) else {
            continue;
        };
        let component_reason = explain_component(
            component,
            provenance,
            profile_components,
            pyproject_component_reasons,
        );
        for requirement in &metadata.provides {
            entries.push(WhyEntry {
                name: requirement.clone(),
                source: component_reason.source.clone(),
                reason: format!("provided by `{component}`: {}", component_reason.reason),
                remove_hint: format!(
                    "remove `{component}` from `components` in robo.nix if `{requirement}` is unnecessary"
                ),
                remediation_hint: format!(
                    "keep `{component}` or another component that provides `{requirement}`"
                ),
            });
        }
    }
    entries
}

fn dedupe_why_entries(entries: Vec<WhyEntry>) -> Vec<WhyEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.name.clone()))
        .collect()
}

fn explain_required_path(kind: &str, path: &str, _provenance: &ProjectProvenance) -> WhyEntry {
    let config_name = if kind == "file" {
        "requiredFiles"
    } else {
        "requiredDirectories"
    };

    WhyEntry {
        name: path.to_string(),
        source: "manual config".to_string(),
        reason: format!("listed in {config_name} in robo.nix"),
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

fn profile_components(manifest: &RuntimeManifest, profile: &str) -> Option<Vec<String>> {
    manifest
        .profiles
        .get(profile)
        .map(|profile| profile.components.clone())
}

fn read_runtime_manifest() -> Option<RuntimeManifest> {
    let manifest_path = env::var("ROBO_NIX_COMPONENT_MANIFEST").ok()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    serde_json::from_str(&manifest).ok()
}

fn read_project_manifest() -> Option<ProjectManifest> {
    let expr = r#"
      let
        spec = import ./robo.nix;
        provenance = spec.provenance or {};
      in builtins.toJSON {
        schemaVersion = if spec ? schemaVersion then toString spec.schemaVersion else null;
        envName = spec.envName or null;
        profile = spec.profile or provenance.profile or null;
        pythonVersion = spec.pythonVersion or null;
        cudaWheelVersion = spec.cudaWheelVersion or null;
        requirements = spec.requirements or [];
        components = spec.components or [];
        requiredDirectories = spec.requiredDirectories or [];
        requiredFiles = spec.requiredFiles or [];
        provenance = {
          profile = provenance.profile or null;
          inferred = provenance.inferred or [];
          componentReasons = provenance.componentReasons or [];
          sourceScripts = provenance.sourceScripts or [];
          suggestions = provenance.suggestions or [];
        };
      }
    "#;
    let output = Command::new("nix")
        .args([
            "--extra-experimental-features",
            "nix-command",
            "--extra-experimental-features",
            "flakes",
            "--no-warn-dirty",
            "--quiet",
            "eval",
            "--json",
            "--impure",
            "--expr",
            expr,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let encoded: String = serde_json::from_slice(&output.stdout).ok()?;
    serde_json::from_str(&encoded).ok()
}

fn component_reasons(
    component_reasons: Vec<ProjectManifestComponentReason>,
) -> HashMap<String, ComponentReason> {
    let mut reasons = HashMap::new();
    for item in component_reasons {
        reasons.insert(
            item.name,
            ComponentReason {
                source: item.source,
                reason: item.reason,
            },
        );
    }
    reasons
}

fn extract_quoted(text: &str) -> Option<&str> {
    let start = text.find('"')?;
    let text = &text[start + 1..];
    let end = text.find('"')?;
    Some(&text[..end])
}

fn default_inference_source() -> String {
    "inference".to_string()
}

fn default_component_reason() -> String {
    "listed in provenance.componentReasons".to_string()
}

fn default_requirement_reason() -> String {
    "listed in requirements".to_string()
}

fn default_requirement_severity() -> String {
    "required".to_string()
}

fn is_cuda_package_name(name: &str) -> bool {
    name.contains("-cu")
        && name
            .rsplit("-cu")
            .next()
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_cuda_wheel_version_from_uv_lock() {
        let lock = r#"
[[package]]
name = "numpydantic"
version = "1.6.7"

[[package]]
name = "nvidia-cudnn-cu12"
version = "9.5.1.17"

[[package]]
name = "nvidia-cuda-runtime-cu12"
version = "12.6.77"
"#;
        assert_eq!(
            infer_cuda_wheel_version_from_uv_lock_text(lock),
            Some("12.6".to_string())
        );
    }

    #[test]
    fn infers_max_cuda_minor_from_uv_lock() {
        let lock = r#"
[[package]]
name = "nvidia-cublas-cu12"
version = "12.5.3"

[[package]]
name = "nvidia-cuda-runtime-cu12"
version = "12.6.77"
"#;
        assert_eq!(
            infer_cuda_wheel_version_from_uv_lock_text(lock),
            Some("12.6".to_string())
        );
    }

    #[test]
    fn parses_cuda_version_from_path() {
        assert_eq!(
            parse_cuda_version_from_path(std::path::Path::new(
                "/nix/store/x-robo-cuda-toolkit-12.8"
            )),
            Some("12.8".to_string())
        );
        assert_eq!(
            parse_cuda_version_from_path(std::path::Path::new(
                "/nix/store/x-cuda-toolkit-12.3"
            )),
            Some("12.3".to_string())
        );
    }

    #[test]
    fn parses_release_line() {
        let output = "Cuda compilation tools, release 12.8, V12.8.0";
        assert_eq!(find_cuda_release_version(output), Some("12.8".to_string()));
    }

    #[test]
    fn parses_nvidia_smi_cuda_version() {
        let output = "| NVIDIA-SMI 550.54.15 Driver Version: 550.54.15 CUDA Version: 12.4 |";
        assert_eq!(
            find_nvidia_smi_cuda_version(output),
            Some("12.4".to_string())
        );
    }

    #[test]
    fn parses_cuda_driver_api_version() {
        assert_eq!(cuda_driver_api_version(12080), Some("12.8".to_string()));
        assert_eq!(cuda_driver_api_version(0), None);
    }

    #[test]
    fn compares_cuda_versions() {
        assert_eq!(cuda_version_less_than("12.4", "12.8"), Some(true));
        assert_eq!(cuda_version_less_than("12.8", "12.8"), Some(false));
        assert_eq!(cuda_version_less_than("12.9", "12.8"), Some(false));
    }

    #[cfg(all(target_os = "linux", not(target_env = "musl")))]
    #[test]
    fn missing_dynamic_library_does_not_close_null_handle() {
        assert!(
            DynamicLibrary::open("/definitely/not/a/real/robo-nix-library.so").is_none()
        );
    }
}
