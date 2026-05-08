use clap::Args;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::{Config, LabelKind, error, inline, label};

pub(crate) mod id {
    pub(crate) const PYTHON_ENV_MISSING: &str = "python.env-missing";
    pub(crate) const PYTHON_ENV_HOST_OWNED: &str = "python.env-host-owned";
    pub(crate) const PYTHON_VERSION_MISMATCH: &str = "python.version-mismatch";
    pub(crate) const PYTHON_PROJECT_FILES_MISSING: &str = "python.project-files-missing";

    pub(crate) const RUNTIME_FILES_MISSING_OR_STALE: &str = "runtime.files-missing-or-stale";
    pub(crate) const RUNTIME_COMPONENTS_INCOMPLETE: &str = "runtime.components-incomplete";
    pub(crate) const RUNTIME_TOOL_MISSING: &str = "runtime.tool-missing";

    pub(crate) const PROJECT_REQUIRED_DIRECTORIES_MISSING: &str =
        "project.required-directories-missing";

    pub(crate) const NATIVE_PYTHON_BUILD_TOOL_SHIM: &str = "native.python-build-tool-shim";

    pub(crate) const CUDA_HOST_NOT_READY: &str = "cuda.host-not-ready";
    pub(crate) const CUDA_DRIVER_NOT_VISIBLE: &str = "cuda.driver-not-visible";
    pub(crate) const CUDA_DRIVER_WHEEL_MISMATCH: &str = "cuda.driver-wheel-mismatch";
    pub(crate) const CUDA_TOOLKIT_NOT_VISIBLE: &str = "cuda.toolkit-not-visible";

    pub(crate) const GRAPHICS_EGL_CONTEXT: &str = "graphics.egl-context";
    pub(crate) const GRAPHICS_MUJOCO_GL_FORCED: &str = "graphics.mujoco-gl-forced";
    pub(crate) const GRAPHICS_SOFTWARE_RENDERER: &str = "graphics.software-renderer";
    pub(crate) const GRAPHICS_PYTHON_GUI_IMPORT: &str = "graphics.python-gui-import";

    pub(crate) const MEDIA_FFMPEG_RUNTIME_MISSING: &str = "media.ffmpeg-runtime-missing";
}

#[derive(Args)]
pub(crate) struct DiagnoseArgs {
    #[arg(value_name = "FILE", help = "Log file to classify, or - for stdin")]
    input: Option<PathBuf>,

    #[arg(long, help = "Emit machine-readable diagnosis JSON")]
    json: bool,
}

#[derive(serde::Serialize)]
struct DiagnoseOutput {
    schema: &'static str,
    matches: Vec<Diagnosis>,
    suggestions: Vec<Suggestion>,
}

impl From<DiagnosisResult> for DiagnoseOutput {
    fn from(result: DiagnosisResult) -> Self {
        Self {
            schema: "robo.diagnosis.v1",
            matches: result.matches,
            suggestions: result.suggestions,
        }
    }
}

struct DiagnosisResult {
    matches: Vec<Diagnosis>,
    suggestions: Vec<Suggestion>,
}

impl DiagnosisResult {
    fn from_text(text: &str) -> Self {
        let matches = diagnose_text(text);
        let suggestions = if matches.is_empty() {
            suggest_text(text)
        } else {
            Vec::new()
        };
        Self {
            matches,
            suggestions,
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct Diagnosis {
    id: &'static str,
    title: &'static str,
    severity: Severity,
    owner: &'static str,
    summary: &'static str,
    next: &'static [&'static str],
    docs: &'static str,
    matched: Vec<&'static str>,
}

#[derive(Clone, serde::Serialize)]
struct Suggestion {
    id: &'static str,
    title: &'static str,
    docs: &'static str,
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }

    fn label_kind(self) -> LabelKind {
        match self {
            Self::Error => LabelKind::Error,
            Self::Warning => LabelKind::Warn,
        }
    }
}

const DOCS_GLIBC: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#glibc-symbol-error";
const DOCS_PYTHON_ENV: &str = "https://ausbxuse.github.io/robo-nix/users/troubleshooting#python-environment-missing-or-host-owned";
const DOCS_PYTHON_FILES: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#python-project-files-missing";
const DOCS_RUNTIME_REVIEW: &str = "https://ausbxuse.github.io/robo-nix/users/troubleshooting#runtime-files-or-components-need-review";
const DOCS_RUNTIME_TOOL: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#runtime-shell-tool-missing";
const DOCS_NATIVE_SHIMS: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#native-build-tool-shims-in-venv";
const DOCS_CUDA_DRIVER: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#cuda-driver-library-not-found";
const DOCS_CUDA_MISMATCH: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#cuda-wheel-and-driver-mismatch";
const DOCS_CUDA_TOOLKIT: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#cuda-toolkit-not-visible";
const DOCS_QT_CMAKE: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#qt-cmake-files-missing";
const DOCS_PYTHON_CMAKE: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#python-cmake-helper-missing";
const DOCS_GRAPHICS: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#egl-or-opengl-context-failure";
const DOCS_PYTHON_GUI: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#python-gui-import-failed";
const DOCS_MEDIA: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#ffmpeg-media-runtime-missing";
const DOCS_LOCAL_SOURCE: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#missing-local-editable-source";
const DOCS_LINUX_HEADERS: &str =
    "https://ausbxuse.github.io/robo-nix/users/troubleshooting#missing-linux-headers";

const GLIBC_MARKERS: &[&str] = &["GLIBC_", "not found", "required by /nix/store/"];
const LOCAL_SOURCE_MARKERS: &[&str] = &["Distribution not found at:", "file://"];
const QT_CMAKE_MARKERS: &[&str] = &["Qt6Config.cmake"];
const PYTHON_CMAKE_MARKERS: &[&str] = &[
    "pybind11Config.cmake",
    "nanobindConfig.cmake",
    "Could not find pybind11",
    "Could not find nanobind",
];
const CUDA_DRIVER_MARKERS: &[&str] = &[
    "libcuda.so.1: cannot open shared object file",
    "CUDA driver library not found",
    "CUDA driver library not visible",
    "libcuda.so.1 was not visible",
    "CUDA_ERROR_NO_DEVICE",
];
const CUDA_MISMATCH_MARKERS: &[&str] = &[
    "CUDA driver version is insufficient",
    "uv.lock expects CUDA",
    "runtime CUDA mismatch",
    "host supports CUDA",
    "CUDA host driver is too old",
    "CUDA host driver mismatch",
    "CUDA mismatch: uv.lock expects",
    "CUDA toolkit version does not match uv.lock",
];
const EGL_MARKERS: &[&str] = &[
    "EGL: Failed to get EGL display",
    "Failed to get EGL display",
    "Failed EGL display",
    "gladLoadGL error",
    "OpenGL platform library has not been loaded",
    "an OpenGL platform library has not been loaded",
    "Wayland: Failed to load libwayland-client",
    "Failed to load libwayland-client",
    "libEGL.so.1 is not visible in the runtime library path",
    "EGL vendor file is missing",
    "Nix libEGL is paired with a non-Nix EGL vendor file",
    "no Wayland or X11 display variable is visible in the runtime shell",
    "graphics session is",
];
const LINUX_HEADERS_MARKERS: &[&str] = &[
    "linux/input.h: No such file or directory",
    "linux/joystick.h: No such file or directory",
];
const PYTHON_ENV_MISSING_MARKERS: &[&str] = &[
    "Python virtualenv is missing",
    "Python environment missing",
];
const PYTHON_ENV_HOST_MARKERS: &[&str] = &[
    "Python virtualenv was created outside robo-nix",
    "Python environment was created outside robo-nix",
];
const PYTHON_VERSION_MISMATCH_MARKERS: &[&str] = &[
    "Python version mismatch",
    "but robo.nix declares",
];
const PYTHON_PROJECT_FILE_MARKERS: &[&str] = &[
    ".python-version is missing",
    "pyproject.toml is missing",
    "pyproject.toml missing",
    "uv.lock missing",
];
const NATIVE_BUILD_TOOL_SHIM_MARKERS: &[&str] = &[
    "Python virtualenv contains native build tool shims",
    "Python environment contains native build tool shims",
];
const RUNTIME_COMPONENT_GAP_MARKERS: &[&str] = &[
    "runtime components may be incomplete",
    "robo may be missing runtime components",
];
const RUNTIME_FILE_MARKERS: &[&str] = &[
    "runtime files need review",
    "generated robo.nix schema is missing",
    "No robo runtime files were found in this project",
];
const REQUIRED_DIRECTORY_MARKERS: &[&str] = &[
    "required directories missing",
    "missing required directory",
];
const CUDA_HOST_MARKERS: &[&str] = &[
    "CUDA requires a Linux host",
    "CUDA environments require a Linux host",
    "could not detect host NVIDIA driver CUDA support",
    "NVIDIA driver stack not found",
];
const CUDA_TOOLKIT_MARKERS: &[&str] = &[
    "CUDA root is not visible in the current shell",
    "CUDA toolkit not visible in this shell",
    "CUDA toolkit version is unknown",
    "CUDA native build surface is incomplete",
    "CUDA_HOME/CUDA_PATH did not point at a toolkit",
    "failed to probe CUDA native build surface",
];
const RUNTIME_TOOL_MARKERS: &[&str] = &[
    "uv is not available in the runtime shell",
    "failed to probe uv in runtime shell",
];
const MUJOCO_GL_MARKERS: &[&str] = &[
    "MuJoCo GL backend is forced",
    "Graphics runtime is blocked by MUJOCO_GL",
];
const SOFTWARE_GRAPHICS_MARKERS: &[&str] = &[
    "LIBGL_ALWAYS_SOFTWARE=1 forces software rendering",
    "OpenGL renderer appears to be software",
];
const PYTHON_GUI_IMPORT_MARKERS: &[&str] = &[
    "PyQt6 GUI import failed",
    "failed to run PyQt6 GUI probe",
    "matplotlib QtAgg backend probe failed",
    "failed to run matplotlib QtAgg probe",
];
const MEDIA_RUNTIME_MARKERS: &[&str] = &[
    "TorchCodec import failed",
    "failed to run TorchCodec import probe",
    "TorchCodec needs FFmpeg shared libraries",
];

pub(crate) fn run(args: DiagnoseArgs, config: Config) -> ExitCode {
    let text = match read_input(args.input.as_ref()) {
        Ok(text) => text,
        Err(err) => {
            error(config, &err);
            return ExitCode::from(2);
        }
    };

    let result = DiagnosisResult::from_text(&text);
    if args.json {
        let output = DiagnoseOutput::from(result);
        match serde_json::to_string_pretty(&output) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                error(config, &format!("failed to encode diagnosis JSON: {err}"));
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    print_result(config, &result);
    ExitCode::SUCCESS
}

fn read_input(input: Option<&PathBuf>) -> Result<String, String> {
    match input {
        Some(path) if path.as_os_str() == "-" => read_stdin(),
        Some(path) => fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display())),
        None if !io::stdin().is_terminal() => read_stdin(),
        None => Err(
            "I need an error log to diagnose. Pass a file, or pipe logs with `<command> 2>&1 | robo diagnose -`."
                .to_string(),
        ),
    }
}

fn read_stdin() -> Result<String, String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|err| format!("failed to read stdin: {err}"))?;
    Ok(text)
}

fn diagnose_text(text: &str) -> Vec<Diagnosis> {
    let normalized = normalize(text);
    let id_matches = diagnose_ids(&normalized);
    if !id_matches.is_empty() {
        return id_matches;
    }

    let mut matches = Vec::new();
    for entry in ENTRIES {
        if let Some(matched) = entry.matcher.matches(&normalized) {
            let severity =
                leading_diagnostic_severity(&normalized).unwrap_or(entry.template.severity);
            matches.push(entry.template.to_diagnosis_with_severity(matched, severity));
        }
    }
    matches
}

fn diagnose_ids(normalized: &str) -> Vec<Diagnosis> {
    let mut matches = Vec::new();
    for entry in ENTRIES {
        if contains_diagnostic_id(normalized, entry.template.id) {
            let severity = explicit_diagnostic_severity(normalized, entry.template.id)
                .unwrap_or(entry.template.severity);
            matches.push(
                entry
                    .template
                    .to_diagnosis_with_severity(vec![entry.template.id], severity),
            );
        }
    }
    matches
}

fn contains_diagnostic_id(text: &str, id: &str) -> bool {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric()
            || character == '.'
            || character == '-'
            || character == '_')
    })
    .any(|token| token == id)
}

fn explicit_diagnostic_severity(text: &str, id: &str) -> Option<Severity> {
    if text.contains(&format!("error[{id}]")) {
        Some(Severity::Error)
    } else if text.contains(&format!("warn[{id}]")) || text.contains(&format!("warning[{id}]")) {
        Some(Severity::Warning)
    } else {
        None
    }
}

fn leading_diagnostic_severity(text: &str) -> Option<Severity> {
    if text.starts_with("error:") {
        Some(Severity::Error)
    } else if text.starts_with("warn:") || text.starts_with("warning:") {
        Some(Severity::Warning)
    } else {
        None
    }
}

fn suggest_text(text: &str) -> Vec<Suggestion> {
    let query_terms = search_terms(&normalize(text));
    if query_terms.len() < 2 {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    for entry in ENTRIES {
        if query_terms
            .iter()
            .all(|term| entry.search_terms.iter().any(|candidate| candidate == term))
        {
            suggestions.push(entry.template.to_suggestion());
        }
    }
    suggestions
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn search_terms(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn contains_normalized(text: &str, marker: &str) -> bool {
    text.contains(&normalize(marker))
}

fn print_result(config: Config, result: &DiagnosisResult) {
    if result.matches.is_empty() {
        if !result.suggestions.is_empty() {
            println!("{}", label(config, "no diagnosis matched", LabelKind::Warn));
            println!();
            println!("{}", label(config, "reason", LabelKind::Status));
            println!(
                "  {}",
                inline(config, "not enough evidence for a known failure match")
            );
            println!("{}", label(config, "possible matches", LabelKind::Status));
            for suggestion in &result.suggestions {
                println!(
                    "  {}  {}",
                    label(config, suggestion.id, LabelKind::Hint),
                    inline(config, suggestion.title)
                );
                println!(
                    "    {:<5} {}",
                    label(config, "docs", LabelKind::Hint),
                    inline(config, suggestion.docs)
                );
            }
            println!("{}", label(config, "try", LabelKind::Status));
            println!(
                "  {}",
                inline(config, "pipe the full error log to `robo diagnose -`")
            );
            println!("  {}", label(config, "robo check --deep", LabelKind::Command));
            return;
        }

        println!("{}", label(config, "no diagnosis matched", LabelKind::Warn));
        println!();
        println!("{}", label(config, "try", LabelKind::Status));
        println!("  {}", label(config, "robo check --deep", LabelKind::Command));
        println!();
        println!("{}", label(config, "docs", LabelKind::Status));
        println!(
            "  {}",
            inline(config, "https://ausbxuse.github.io/robo-nix/users/troubleshooting")
        );
        return;
    }

    for (index, diagnosis) in result.matches.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!(
            "{} {}",
            label(
                config,
                &format!("{}[{}]:", diagnosis.severity.label(), diagnosis.id),
                diagnosis.severity.label_kind()
            ),
            inline(config, diagnosis.title)
        );
        println!();
        println!("{}", label(config, "problem", LabelKind::Status));
        println!("  {}", inline(config, diagnosis.summary));
        println!();
        println!("{}", label(config, "evidence", LabelKind::Status));
        for marker in display_markers(&diagnosis.matched) {
            println!("  {}", inline(config, marker));
        }
        println!();
        println!("{}", label(config, "owner", LabelKind::Status));
        println!("  {}", inline(config, diagnosis.owner));
        println!();
        println!("{}", label(config, "try", LabelKind::Status));
        for command in diagnosis.next {
            println!("  {}", label(config, command, LabelKind::Command));
        }
        println!();
        println!("{}", label(config, "docs", LabelKind::Status));
        println!("  {}", inline(config, diagnosis.docs));
    }
}

fn display_markers(markers: &[&'static str]) -> Vec<&'static str> {
    markers
        .iter()
        .copied()
        .filter(|marker| {
            !markers
                .iter()
                .any(|other| {
                    other.len() > marker.len() && contains_normalized(&normalize(other), marker)
                })
        })
        .collect()
}

#[derive(Clone, Copy)]
struct DiagnosisTemplate {
    id: &'static str,
    title: &'static str,
    severity: Severity,
    owner: &'static str,
    summary: &'static str,
    next: &'static [&'static str],
    docs: &'static str,
}

impl DiagnosisTemplate {
    fn to_diagnosis_with_severity(
        self,
        matched: Vec<&'static str>,
        severity: Severity,
    ) -> Diagnosis {
        Diagnosis {
            id: self.id,
            title: self.title,
            severity,
            owner: self.owner,
            summary: self.summary,
            next: self.next,
            docs: self.docs,
            matched,
        }
    }

    fn to_suggestion(self) -> Suggestion {
        Suggestion {
            id: self.id,
            title: self.title,
            docs: self.docs,
        }
    }
}

#[derive(Clone, Copy)]
struct FailureEntry {
    template: DiagnosisTemplate,
    matcher: Matcher,
    search_terms: &'static [&'static str],
}

#[derive(Clone, Copy)]
enum Matcher {
    All(&'static [&'static str]),
    Any(&'static [&'static str]),
    AnyOrPair {
        any: &'static [&'static str],
        required: &'static str,
        one_of: &'static [&'static str],
    },
    AnyPair {
        required: &'static str,
        one_of: &'static [&'static str],
    },
}

impl Matcher {
    fn matches(&self, text: &str) -> Option<Vec<&'static str>> {
        match self {
            Self::All(markers) => {
                let matched = matched_markers(text, markers);
                (matched.len() == markers.len()).then_some(matched)
            }
            Self::Any(markers) => {
                let matched = matched_markers(text, markers);
                (!matched.is_empty()).then_some(matched)
            }
            Self::AnyOrPair {
                any,
                required,
                one_of,
            } => {
                let matched = matched_markers(text, any);
                if !matched.is_empty() {
                    return Some(matched);
                }
                Self::AnyPair { required, one_of }.matches(text)
            }
            Self::AnyPair { required, one_of } => {
                if !contains_normalized(text, required) {
                    return None;
                }
                let mut matched = matched_markers(text, one_of);
                if matched.is_empty() {
                    return None;
                }
                matched.insert(0, required);
                Some(matched)
            }
        }
    }
}

fn matched_markers(text: &str, markers: &'static [&'static str]) -> Vec<&'static str> {
    markers
        .iter()
        .copied()
        .filter(|marker| contains_normalized(text, marker))
        .collect()
}

const ENTRIES: &[FailureEntry] = &[
    FailureEntry {
        template: DiagnosisTemplate {
            id: "python.glibc-abi-mix",
            title: "Host Python/glibc is mixing with Nix native libraries",
            severity: Severity::Error,
            owner: "Python environment state plus native runtime ABI alignment",
            summary: "A binary loaded from the Nix store requires a newer glibc symbol than the active host process provides.",
            next: &["robo shell", "uv venv --python \"$ROBO_NIX_PYTHON\" --clear", "uv sync"],
            docs: DOCS_GLIBC,
        },
        matcher: Matcher::All(GLIBC_MARKERS),
        search_terms: &["glibc", "version", "found", "nix", "store", "libstdc"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "python.local-editable-missing",
            title: "A local editable Python source is missing",
            severity: Severity::Error,
            owner: "Project checkout, submodules, vendored sources, or dependency declarations",
            summary: "The Python dependency metadata points at a local file URL that does not exist in this checkout.",
            next: &["git submodule update --init --recursive"],
            docs: DOCS_LOCAL_SOURCE,
        },
        matcher: Matcher::All(LOCAL_SOURCE_MARKERS),
        search_terms: &["distribution", "found", "file", "third", "party", "editable"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::PYTHON_ENV_MISSING,
            title: "Python virtualenv is missing",
            severity: Severity::Error,
            owner: "Project uv environment state",
            summary: "The runtime exists, but uv has not created or synced the project virtualenv yet.",
            next: &["robo shell", "uv sync"],
            docs: DOCS_PYTHON_ENV,
        },
        matcher: Matcher::Any(PYTHON_ENV_MISSING_MARKERS),
        search_terms: &["python", "virtualenv", "missing", "uv", "sync"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::PYTHON_ENV_HOST_OWNED,
            title: "Python virtualenv was created outside robo-nix",
            severity: Severity::Error,
            owner: "Project uv environment state plus Python/native ABI alignment",
            summary: "The active .venv is backed by a host Python instead of the Nix-provided robo runtime interpreter.",
            next: &["robo shell", "uv venv --python \"$ROBO_NIX_PYTHON\" --clear", "uv sync"],
            docs: DOCS_PYTHON_ENV,
        },
        matcher: Matcher::Any(PYTHON_ENV_HOST_MARKERS),
        search_terms: &["python", "virtualenv", "host", "robo", "nix", "outside"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::PYTHON_VERSION_MISMATCH,
            title: "Python version contract is inconsistent",
            severity: Severity::Error,
            owner: "Project Python version contract",
            summary: "pyproject.toml, .python-version, and robo.nix do not agree on the Python version.",
            next: &["edit pythonVersion in robo.nix", "update .python-version", "robo check"],
            docs: DOCS_PYTHON_FILES,
        },
        matcher: Matcher::Any(PYTHON_VERSION_MISMATCH_MARKERS),
        search_terms: &["python", "version", "mismatch", "pyproject", "robo"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::PYTHON_PROJECT_FILES_MISSING,
            title: "Python project contract files are missing",
            severity: Severity::Error,
            owner: "Project Python/uv contract",
            summary: "robo expected the project-owned Python files that uv uses for version selection, dependency locking, or package metadata.",
            next: &["robo init .", "uv sync"],
            docs: DOCS_PYTHON_FILES,
        },
        matcher: Matcher::Any(PYTHON_PROJECT_FILE_MARKERS),
        search_terms: &["python", "pyproject", "lock", "missing"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::RUNTIME_FILES_MISSING_OR_STALE,
            title: "Runtime files are missing or stale",
            severity: Severity::Warning,
            owner: "Generated robo runtime files",
            summary: "The generated runtime files are missing, stale, or use a schema this CLI cannot trust.",
            next: &["robo init . --force", "robo check"],
            docs: DOCS_RUNTIME_REVIEW,
        },
        matcher: Matcher::Any(RUNTIME_FILE_MARKERS),
        search_terms: &["runtime", "files", "missing", "stale", "schema"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::RUNTIME_COMPONENTS_INCOMPLETE,
            title: "Runtime components may be incomplete",
            severity: Severity::Warning,
            owner: "robo.nix runtime component selection",
            summary: "The project metadata suggests runtime components that are not selected in robo.nix.",
            next: &["robo init . --force", "review robo.nix", "robo check"],
            docs: DOCS_RUNTIME_REVIEW,
        },
        matcher: Matcher::Any(RUNTIME_COMPONENT_GAP_MARKERS),
        search_terms: &["runtime", "components", "incomplete", "missing"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::PROJECT_REQUIRED_DIRECTORIES_MISSING,
            title: "Required project directories are missing",
            severity: Severity::Warning,
            owner: "Project checkout layout",
            summary: "robo.nix declares workspace directories that are not present in this checkout.",
            next: &["create the missing directories or edit requiredDirectories in robo.nix", "robo check"],
            docs: DOCS_RUNTIME_REVIEW,
        },
        matcher: Matcher::Any(REQUIRED_DIRECTORY_MARKERS),
        search_terms: &["required", "directories", "missing", "workspace"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "native.qt6-cmake-missing",
            title: "Qt 6 CMake package files are missing from the runtime",
            severity: Severity::Error,
            owner: "Nix runtime dependencies",
            summary: "CMake looked for Qt6Config.cmake while building native code.",
            next: &["add qt6 to robo.nix components", "robo build"],
            docs: DOCS_QT_CMAKE,
        },
        matcher: Matcher::Any(QT_CMAKE_MARKERS),
        search_terms: &["qt6", "cmake", "config", "package", "native"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "python.cmake-helper-missing",
            title: "A Python-owned CMake helper package is missing",
            severity: Severity::Error,
            owner: "Project Python dependency policy",
            summary: "The native build expected CMake files from a Python package such as pybind11 or nanobind, but the project did not make that helper visible to the package build.",
            next: &[
                "declare the helper package in pyproject.toml, for example pybind11 or nanobind",
                "if the failing local package uses setup.py/CMake without build-system requirements, add it to tool.uv.no-build-isolation-package",
                "rerun the uv sync command documented by the project",
            ],
            docs: DOCS_PYTHON_CMAKE,
        },
        matcher: Matcher::Any(PYTHON_CMAKE_MARKERS),
        search_terms: &["pybind11", "nanobind", "cmake", "config", "python"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::CUDA_DRIVER_NOT_VISIBLE,
            title: "CUDA driver library is not visible",
            severity: Severity::Error,
            owner: "Host GPU driver integration",
            summary: "The runtime could not see the host NVIDIA driver library or device.",
            next: &["robo check cuda --verbose"],
            docs: DOCS_CUDA_DRIVER,
        },
        matcher: Matcher::Any(CUDA_DRIVER_MARKERS),
        search_terms: &["cuda", "driver", "libcuda", "device", "visible"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::CUDA_HOST_NOT_READY,
            title: "CUDA host driver support is not ready",
            severity: Severity::Error,
            owner: "Host GPU driver integration",
            summary: "The selected runtime needs a Linux NVIDIA host driver, but robo could not confirm the host CUDA driver surface.",
            next: &["robo check cuda --verbose"],
            docs: DOCS_CUDA_DRIVER,
        },
        matcher: Matcher::Any(CUDA_HOST_MARKERS),
        search_terms: &["cuda", "host", "driver", "nvidia", "linux"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::CUDA_DRIVER_WHEEL_MISMATCH,
            title: "CUDA wheels require a newer or different driver ABI",
            severity: Severity::Error,
            owner: "Host driver version or project Python dependency lock",
            summary: "The selected Python CUDA wheels do not align with the CUDA driver API visible on this host.",
            next: &["robo check cuda --verbose"],
            docs: DOCS_CUDA_MISMATCH,
        },
        matcher: Matcher::Any(CUDA_MISMATCH_MARKERS),
        search_terms: &["cuda", "driver", "version", "insufficient", "expects", "mismatch"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::CUDA_TOOLKIT_NOT_VISIBLE,
            title: "CUDA toolkit is not visible in the runtime",
            severity: Severity::Error,
            owner: "Nix CUDA toolkit runtime",
            summary: "Native CUDA extension builds need the Nix-owned toolkit surface: nvcc, headers, and libcudart link support.",
            next: &["add cuda-toolkit to robo.nix components", "robo check --deep"],
            docs: DOCS_CUDA_TOOLKIT,
        },
        matcher: Matcher::Any(CUDA_TOOLKIT_MARKERS),
        search_terms: &["cuda", "toolkit", "native", "build", "surface"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::RUNTIME_TOOL_MISSING,
            title: "Runtime shell tool is missing",
            severity: Severity::Error,
            owner: "Nix runtime shell",
            summary: "A tool that robo expects inside the realized runtime shell was not available or could not be probed.",
            next: &["robo check --deep", "robo build"],
            docs: DOCS_RUNTIME_TOOL,
        },
        matcher: Matcher::Any(RUNTIME_TOOL_MARKERS),
        search_terms: &["runtime", "tool", "missing", "uv"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::GRAPHICS_EGL_CONTEXT,
            title: "EGL/OpenGL context creation failed",
            severity: Severity::Error,
            owner: "Host graphics/display integration plus selected runtime graphics libraries",
            summary: "The graphics stack could not create an OpenGL context through EGL, GLVND, Wayland, or X11.",
            next: &["robo check graphics --verbose"],
            docs: DOCS_GRAPHICS,
        },
        matcher: Matcher::Any(EGL_MARKERS),
        search_terms: &[
            "egl",
            "display",
            "failed",
            "opengl",
            "context",
            "gladloadgl",
            "wayland",
            "libwayland",
        ],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::GRAPHICS_MUJOCO_GL_FORCED,
            title: "MuJoCo GL backend is forced",
            severity: Severity::Warning,
            owner: "User shell environment",
            summary: "MUJOCO_GL is forcing a graphics backend that can block desktop GLFW viewers inside the robo runtime.",
            next: &["unset MUJOCO_GL", "robo check graphics --verbose"],
            docs: DOCS_GRAPHICS,
        },
        matcher: Matcher::Any(MUJOCO_GL_MARKERS),
        search_terms: &["mujoco", "gl", "graphics", "forced"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::GRAPHICS_SOFTWARE_RENDERER,
            title: "Graphics runtime is using software rendering",
            severity: Severity::Warning,
            owner: "Host graphics/device visibility",
            summary: "OpenGL is being forced or detected as software rendering, which usually breaks simulator viewers or makes them unusably slow.",
            next: &["unset LIBGL_ALWAYS_SOFTWARE", "robo check graphics --verbose"],
            docs: DOCS_GRAPHICS,
        },
        matcher: Matcher::Any(SOFTWARE_GRAPHICS_MARKERS),
        search_terms: &["opengl", "software", "renderer", "graphics"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::GRAPHICS_PYTHON_GUI_IMPORT,
            title: "Python GUI import failed in the runtime",
            severity: Severity::Error,
            owner: "Project Python dependencies plus Nix graphics/runtime libraries",
            summary: "A Python GUI/backend probe failed after entering the runtime, usually because Qt bindings or native display/OpenGL libraries are incomplete.",
            next: &["uv sync", "review qt6 and desktop-gl components in robo.nix", "robo check --deep"],
            docs: DOCS_PYTHON_GUI,
        },
        matcher: Matcher::Any(PYTHON_GUI_IMPORT_MARKERS),
        search_terms: &["pyqt", "matplotlib", "qtagg", "gui", "graphics"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::MEDIA_FFMPEG_RUNTIME_MISSING,
            title: "FFmpeg media runtime is missing or incomplete",
            severity: Severity::Error,
            owner: "Nix media runtime libraries",
            summary: "A video/media Python package failed because its FFmpeg shared-library runtime is not available or not aligned.",
            next: &["add media to robo.nix components", "robo check --deep"],
            docs: DOCS_MEDIA,
        },
        matcher: Matcher::Any(MEDIA_RUNTIME_MARKERS),
        search_terms: &["torchcodec", "ffmpeg", "media", "runtime"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "native.linux-headers-missing",
            title: "Linux userspace headers are missing",
            severity: Severity::Error,
            owner: "Nix runtime dependencies",
            summary: "A native extension included Linux kernel userspace headers that are not in the runtime.",
            next: &["add linux-headers to robo.nix components", "robo build"],
            docs: DOCS_LINUX_HEADERS,
        },
        matcher: Matcher::Any(LINUX_HEADERS_MARKERS),
        search_terms: &["linux", "input", "joystick", "headers", "directory"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: id::NATIVE_PYTHON_BUILD_TOOL_SHIM,
            title: "Python-owned native build tool shim is crossing the ABI boundary",
            severity: Severity::Warning,
            owner: "Project build invocation plus Python/native boundary",
            summary: "A Python-owned build-tool executable is present where the runtime expects Nix-owned native build tools.",
            next: &["robo shell", "which cmake", "which ninja"],
            docs: DOCS_NATIVE_SHIMS,
        },
        matcher: Matcher::AnyOrPair {
            any: NATIVE_BUILD_TOOL_SHIM_MARKERS,
            required: "GLIBC_",
            one_of: &[".venv/bin/cmake", ".venv/bin/ninja"],
        },
        search_terms: &["venv", "cmake", "ninja", "glibc", "shim"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn ids(text: &str) -> Vec<&'static str> {
        diagnose_text(text)
            .into_iter()
            .map(|diagnosis| diagnosis.id)
            .collect()
    }

    fn first_match(text: &str) -> Diagnosis {
        diagnose_text(text)
            .into_iter()
            .next()
            .expect("expected a diagnosis")
    }

    fn suggestion_ids(text: &str) -> Vec<&'static str> {
        suggest_text(text)
            .into_iter()
            .map(|suggestion| suggestion.id)
            .collect()
    }

    fn diagnose_output(text: &str) -> DiagnoseOutput {
        DiagnoseOutput::from(DiagnosisResult::from_text(text))
    }

    #[test]
    fn diagnosis_entries_have_unique_ids_and_specific_docs() {
        let mut ids = BTreeSet::new();
        for entry in ENTRIES {
            assert!(ids.insert(entry.template.id), "duplicate diagnosis id");
            assert!(
                entry.template.docs.starts_with("https://ausbxuse.github.io/robo-nix/users/"),
                "diagnosis docs should link to user docs: {}",
                entry.template.id
            );
            assert!(
                entry.template.docs.contains('#'),
                "diagnosis docs should link to a specific section: {}",
                entry.template.id
            );
        }
    }

    #[test]
    fn explicit_diagnostic_prefix_can_override_default_severity() {
        let warning = first_match(
            "warn[runtime.files-missing-or-stale]: robo.nix schema version is missing",
        );
        let error = first_match(
            "error[runtime.files-missing-or-stale]: required file is missing: robo.nix",
        );

        assert!(matches!(warning.severity, Severity::Warning));
        assert!(matches!(error.severity, Severity::Error));
    }

    #[test]
    fn leading_robo_status_prefix_sets_marker_match_severity() {
        let warning = first_match("warn: TorchCodec import failed");

        assert_eq!(warning.id, "media.ffmpeg-runtime-missing");
        assert!(matches!(warning.severity, Severity::Warning));
    }

    #[test]
    fn diagnoses_glibc_abi_mix_only_with_nix_store_requirement() {
        let text = "/lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.38' not found \
            (required by /nix/store/41ym-gcc-lib/lib/libstdc++.so.6)";

        assert!(ids(text).contains(&"python.glibc-abi-mix"));
        assert!(!ids("GLIBC_2.38 not found").contains(&"python.glibc-abi-mix"));
    }

    #[test]
    fn diagnoses_local_editable_source_missing() {
        let text = "Distribution not found at: file:///repo/third_party/pkg";

        assert_eq!(ids(text), vec!["python.local-editable-missing"]);
    }

    #[test]
    fn diagnoses_qt6_cmake_missing() {
        let text = "Could not find a package configuration file provided by \"Qt6\" with \
            any of the following names: Qt6Config.cmake";

        assert_eq!(ids(text), vec!["native.qt6-cmake-missing"]);
    }

    #[test]
    fn diagnoses_python_owned_cmake_helpers() {
        assert_eq!(
            ids("Could not find pybind11Config.cmake"),
            vec!["python.cmake-helper-missing"]
        );
        assert_eq!(
            ids("Could not find nanobindConfig.cmake"),
            vec!["python.cmake-helper-missing"]
        );
    }

    #[test]
    fn diagnoses_cuda_driver_visibility_and_mismatch() {
        assert_eq!(
            ids("libcuda.so.1: cannot open shared object file"),
            vec!["cuda.driver-not-visible"]
        );
        assert_eq!(
            ids("CUDA driver version is insufficient for CUDA runtime version"),
            vec!["cuda.driver-wheel-mismatch"]
        );
    }

    #[test]
    fn diagnoses_egl_context_failures_from_specific_and_short_messages() {
        assert_eq!(
            ids("GLFWError: (65542) b'EGL: Failed to get EGL display: Success'"),
            vec!["graphics.egl-context"]
        );
        assert_eq!(
            ids("Failed to get EGL display"),
            vec!["graphics.egl-context"]
        );
        assert_eq!(ids("Failed EGL display"), vec!["graphics.egl-context"]);
    }

    #[test]
    fn reports_only_actual_matched_markers() {
        let diagnosis = first_match("Failed to get EGL display");

        assert_eq!(diagnosis.matched, vec!["Failed to get EGL display"]);
    }

    #[test]
    fn matches_case_insensitively_and_with_collapsed_whitespace() {
        let diagnosis = first_match("failed to get\nEGL   display");

        assert_eq!(diagnosis.id, "graphics.egl-context");
    }

    #[test]
    fn diagnoses_linux_headers_missing() {
        assert_eq!(
            ids("fatal error: linux/input.h: No such file or directory"),
            vec!["native.linux-headers-missing"]
        );
    }

    #[test]
    fn diagnoses_native_build_tool_shim_when_glibc_error_is_present() {
        let text = ".venv/bin/cmake: /lib/libc.so.6: version `GLIBC_2.38' not found";

        assert_eq!(ids(text), vec!["native.python-build-tool-shim"]);
    }

    #[test]
    fn diagnoses_native_build_tool_shim_warning_from_robo_check() {
        let text = "warn: Python virtualenv contains native build tool shims: cmake";

        assert_eq!(ids(text), vec!["native.python-build-tool-shim"]);
    }

    #[test]
    fn diagnosis_id_in_robo_output_wins_over_text_matching() {
        let text = "warn[native.python-build-tool-shim]: Python virtualenv contains native build tool shims: cmake";
        let diagnosis = first_match(text);

        assert_eq!(diagnosis.id, "native.python-build-tool-shim");
        assert_eq!(diagnosis.matched, vec!["native.python-build-tool-shim"]);
    }

    #[test]
    fn diagnosis_id_in_status_output_is_enough() {
        let text = "! environment: native build tool shims: cmake [native.python-build-tool-shim]";

        assert_eq!(ids(text), vec!["native.python-build-tool-shim"]);
    }

    #[test]
    fn diagnoses_robo_check_python_contract_lines() {
        assert_eq!(
            ids("warn: Python virtualenv is missing"),
            vec!["python.env-missing"]
        );
        assert_eq!(
            ids("error: Python virtualenv was created outside robo-nix"),
            vec!["python.env-host-owned"]
        );
        assert_eq!(
            ids("error: pyproject.toml requires Python 3.12 but robo.nix declares 3.11"),
            vec!["python.version-mismatch"]
        );
        assert_eq!(
            ids("warn: .python-version is missing"),
            vec!["python.project-files-missing"]
        );
        assert_eq!(
            ids("! project: pyproject.toml missing"),
            vec!["python.project-files-missing"]
        );
        assert!(ids("ok: pyproject.toml requires Python 3.11").is_empty());
    }

    #[test]
    fn diagnoses_robo_status_runtime_lines() {
        assert_eq!(
            ids("! runtime: runtime components may be incomplete"),
            vec!["runtime.components-incomplete"]
        );
        assert_eq!(
            ids("! runtime: required directories missing"),
            vec!["project.required-directories-missing"]
        );
        assert_eq!(
            ids("! runtime: runtime files need review"),
            vec!["runtime.files-missing-or-stale"]
        );
    }

    #[test]
    fn diagnoses_robo_check_cuda_lines() {
        assert_eq!(
            ids("error: could not detect host NVIDIA driver CUDA support"),
            vec!["cuda.host-not-ready"]
        );
        assert_eq!(
            ids("! environment: CUDA host driver is too old"),
            vec!["cuda.driver-wheel-mismatch"]
        );
        assert_eq!(
            ids("warn: libcuda.so.1 was not visible through ROBO_NIX_LIBCUDA_PATH"),
            vec!["cuda.driver-not-visible"]
        );
        assert_eq!(
            ids("error: CUDA mismatch: uv.lock expects 12.4, runtime reports 12.2"),
            vec!["cuda.driver-wheel-mismatch"]
        );
        assert_eq!(
            ids("error: CUDA native build surface is incomplete"),
            vec!["cuda.toolkit-not-visible"]
        );
        assert_eq!(
            ids("! environment: CUDA toolkit version is unknown"),
            vec!["cuda.toolkit-not-visible"]
        );
        assert_eq!(
            ids("error: uv is not available in the runtime shell"),
            vec!["runtime.tool-missing"]
        );
    }

    #[test]
    fn diagnoses_robo_check_graphics_and_media_lines() {
        assert_eq!(
            ids("error: libEGL.so.1 is not visible in the runtime library path"),
            vec!["graphics.egl-context"]
        );
        assert_eq!(
            ids("warn: LIBGL_ALWAYS_SOFTWARE=1 forces software rendering"),
            vec!["graphics.software-renderer"]
        );
        assert_eq!(
            ids("! environment: MuJoCo GL backend is forced"),
            vec!["graphics.mujoco-gl-forced"]
        );
        assert_eq!(
            ids("warn: PyQt6 GUI import failed"),
            vec!["graphics.python-gui-import"]
        );
        assert_eq!(
            ids("warn: TorchCodec import failed"),
            vec!["media.ffmpeg-runtime-missing"]
        );
    }

    #[test]
    fn unknown_text_has_no_match() {
        assert!(ids("some project-specific pytest failure").is_empty());
    }

    #[test]
    fn short_search_phrases_return_possible_matches_not_diagnoses() {
        assert!(ids("EGL display").is_empty());
        assert_eq!(suggestion_ids("EGL display"), vec!["graphics.egl-context"]);
    }

    #[test]
    fn confident_output_has_no_suggestions() {
        let output = diagnose_output("Failed EGL display");

        assert_eq!(output.matches[0].id, "graphics.egl-context");
        assert!(output.suggestions.is_empty());
    }
}
