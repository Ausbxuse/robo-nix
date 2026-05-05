use clap::Args;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::{Config, LabelKind, error, inline, label};

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
    agent_handoff: Option<AgentHandoff>,
}

impl From<DiagnosisResult> for DiagnoseOutput {
    fn from(result: DiagnosisResult) -> Self {
        let agent_handoff = result.agent_handoff();
        Self {
            schema: "robo.diagnosis.v1",
            matches: result.matches,
            suggestions: result.suggestions,
            agent_handoff,
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
        let suggestions = matches.is_empty().then(|| suggest_text(text)).unwrap_or_default();
        Self {
            matches,
            suggestions,
        }
    }

    fn agent_handoff(&self) -> Option<AgentHandoff> {
        self.matches.is_empty().then_some(AGENT_HANDOFF)
    }
}

#[derive(Clone, serde::Serialize)]
struct Diagnosis {
    id: &'static str,
    title: &'static str,
    confidence: Confidence,
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
    reason: &'static str,
    docs: &'static str,
}

#[derive(Clone, Copy, serde::Serialize)]
struct AgentHandoff {
    reason: &'static str,
    include: &'static [&'static str],
    commands: &'static [&'static str],
    prompt: &'static str,
}

const AGENT_HANDOFF: AgentHandoff = AgentHandoff {
    reason: "robo could not make a high-confidence diagnosis from the provided text",
    include: &[
        "the full command that failed",
        "the complete error log, not only one line",
        "robo diagnose --json output",
        "robo check --deep output",
        "robo.nix and pyproject.toml when they are relevant",
    ],
    commands: &[
        "<failing command> 2>&1 | tee /tmp/robo-error.log",
        "robo diagnose --json /tmp/robo-error.log",
        "robo check --deep 2>&1 | tee /tmp/robo-check.log",
    ],
    prompt: "Ask the agent to classify the failure by owner: uv/Python, Nix runtime, host GPU/driver, project bootstrap, or project dependency policy.",
};

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum Confidence {
    High,
}

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
    "CUDA_ERROR_NO_DEVICE",
];
const CUDA_MISMATCH_MARKERS: &[&str] = &[
    "CUDA driver version is insufficient",
    "uv.lock expects CUDA",
    "runtime CUDA mismatch",
    "host supports CUDA",
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
];
const LINUX_HEADERS_MARKERS: &[&str] = &[
    "linux/input.h: No such file or directory",
    "linux/joystick.h: No such file or directory",
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
    let mut matches = Vec::new();
    for entry in ENTRIES {
        if let Some(matched) = entry.matcher.matches(&normalized) {
            matches.push(entry.template.to_diagnosis(matched));
        }
    }
    matches
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
            suggestions.push(entry.template.to_suggestion("partial search-term match"));
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
            println!(
                "{} {}",
                label(config, "no diagnosis:", LabelKind::Warn),
                inline(config, "not enough evidence for a high-confidence failure match")
            );
            println!("{}", label(config, "possible matches:", LabelKind::Status));
            for suggestion in &result.suggestions {
                println!(
                    "  {} {}",
                    label(config, suggestion.id, LabelKind::Why),
                    inline(config, suggestion.title)
                );
                println!(
                    "    {} {}",
                    label(config, "reason:", LabelKind::Hint),
                    inline(config, suggestion.reason)
                );
                println!(
                    "    {} {}",
                    label(config, "docs:", LabelKind::Hint),
                    inline(config, suggestion.docs)
                );
            }
            println!(
                "{} {}",
                label(config, "next:", LabelKind::Status),
                inline(config, "pipe the full error log to `robo diagnose -` or run `robo check --deep`")
            );
            print_agent_handoff(config);
            return;
        }

        println!(
            "{}",
            label(config, "no known runtime failure matched", LabelKind::Warn)
        );
        println!(
            "{} {}",
            label(config, "next:", LabelKind::Status),
            inline(config, "run `robo check --deep` for current runtime probes")
        );
        println!(
            "{} {}",
            label(config, "docs:", LabelKind::Hint),
            inline(config, "`https://ausbxuse.github.io/robo-nix/users/failure-guide`")
        );
        print_agent_handoff(config);
        return;
    }

    for (index, diagnosis) in result.matches.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!(
            "{} {}",
            label(config, "diagnosis:", LabelKind::Status),
            inline(config, diagnosis.title)
        );
        println!(
            "{} {}",
            label(config, "id:", LabelKind::Hint),
            diagnosis.id
        );
        println!(
            "{} {}",
            label(config, "confidence:", LabelKind::Ok),
            "high"
        );
        println!(
            "{} {}",
            label(config, "owner:", LabelKind::Status),
            inline(config, diagnosis.owner)
        );
        println!(
            "{} {}",
            label(config, "problem:", LabelKind::Error),
            inline(config, diagnosis.summary)
        );
        println!("{}", label(config, "matched:", LabelKind::Hint));
        for marker in &diagnosis.matched {
            println!("  {}", inline(config, marker));
        }
        println!("{}", label(config, "next:", LabelKind::Status));
        for command in diagnosis.next {
            println!("  {}", label(config, command, LabelKind::Command));
        }
        println!(
            "{} {}",
            label(config, "docs:", LabelKind::Hint),
            inline(config, diagnosis.docs)
        );
    }
}

fn print_agent_handoff(config: Config) {
    println!("{}", label(config, "agent handoff:", LabelKind::Status));
    println!(
        "  {} {}",
        label(config, "include:", LabelKind::Hint),
        inline(config, "full failing command, complete log, and robo context")
    );
    for command in AGENT_HANDOFF.commands {
        println!("  {}", label(config, command, LabelKind::Command));
    }
    println!(
        "  {} {}",
        label(config, "ask:", LabelKind::Hint),
        inline(config, AGENT_HANDOFF.prompt)
    );
}

#[derive(Clone, Copy)]
struct DiagnosisTemplate {
    id: &'static str,
    title: &'static str,
    confidence: Confidence,
    owner: &'static str,
    summary: &'static str,
    next: &'static [&'static str],
    docs: &'static str,
}

impl DiagnosisTemplate {
    fn to_diagnosis(self, matched: Vec<&'static str>) -> Diagnosis {
        Diagnosis {
            id: self.id,
            title: self.title,
            confidence: self.confidence,
            owner: self.owner,
            summary: self.summary,
            next: self.next,
            docs: self.docs,
            matched,
        }
    }

    fn to_suggestion(self, reason: &'static str) -> Suggestion {
        Suggestion {
            id: self.id,
            title: self.title,
            reason,
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
            confidence: Confidence::High,
            owner: "Python environment state plus native runtime ABI alignment",
            summary: "A binary loaded from the Nix store requires a newer glibc symbol than the active host process provides.",
            next: &["robo shell", "uv venv --clear", "uv sync"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#glibc-version-not-found",
        },
        matcher: Matcher::All(GLIBC_MARKERS),
        search_terms: &["glibc", "version", "found", "nix", "store", "libstdc"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "python.local-editable-missing",
            title: "A local editable Python source is missing",
            confidence: Confidence::High,
            owner: "Project checkout, submodules, vendored sources, or dependency declarations",
            summary: "The Python dependency metadata points at a local file URL that does not exist in this checkout.",
            next: &["git submodule update --init --recursive"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#missing-local-editable-source",
        },
        matcher: Matcher::All(LOCAL_SOURCE_MARKERS),
        search_terms: &["distribution", "found", "file", "third", "party", "editable"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "native.qt6-cmake-missing",
            title: "Qt 6 CMake package files are missing from the runtime",
            confidence: Confidence::High,
            owner: "Nix runtime dependencies",
            summary: "CMake looked for Qt6Config.cmake while building native code.",
            next: &["add qt6 to robo.nix components", "robo up"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#qt-cmake-package-missing",
        },
        matcher: Matcher::Any(QT_CMAKE_MARKERS),
        search_terms: &["qt6", "cmake", "config", "package", "native"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "python.cmake-helper-missing",
            title: "A Python-owned CMake helper package is missing",
            confidence: Confidence::High,
            owner: "Project Python dependency policy",
            summary: "The native build expected CMake files from a Python package such as pybind11 or nanobind, but the project did not make that helper visible to the package build.",
            next: &[
                "declare the helper package in pyproject.toml, for example pybind11 or nanobind",
                "if the failing local package uses setup.py/CMake without build-system requirements, add it to tool.uv.no-build-isolation-package",
                "rerun the uv sync command documented by the project",
            ],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#python-owned-cmake-helper-missing",
        },
        matcher: Matcher::Any(PYTHON_CMAKE_MARKERS),
        search_terms: &["pybind11", "nanobind", "cmake", "config", "python"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "cuda.driver-not-visible",
            title: "CUDA driver library is not visible",
            confidence: Confidence::High,
            owner: "Host GPU driver integration",
            summary: "The runtime could not see the host NVIDIA driver library or device.",
            next: &["robo check cuda --verbose"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#cuda-driver-not-visible",
        },
        matcher: Matcher::Any(CUDA_DRIVER_MARKERS),
        search_terms: &["cuda", "driver", "libcuda", "device", "visible"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "cuda.driver-wheel-mismatch",
            title: "CUDA wheels require a newer or different driver ABI",
            confidence: Confidence::High,
            owner: "Host driver version or project Python dependency lock",
            summary: "The selected Python CUDA wheels do not align with the CUDA driver API visible on this host.",
            next: &["robo check cuda --verbose"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#cuda-wheel-and-driver-mismatch",
        },
        matcher: Matcher::Any(CUDA_MISMATCH_MARKERS),
        search_terms: &["cuda", "driver", "version", "insufficient", "expects", "mismatch"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "graphics.egl-context",
            title: "EGL/OpenGL context creation failed",
            confidence: Confidence::High,
            owner: "Host graphics/display integration plus selected runtime graphics libraries",
            summary: "The graphics stack could not create an OpenGL context through EGL, GLVND, Wayland, or X11.",
            next: &["robo check graphics --verbose"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#egl-or-opengl-context-failure",
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
            id: "native.linux-headers-missing",
            title: "Linux userspace headers are missing",
            confidence: Confidence::High,
            owner: "Nix runtime dependencies",
            summary: "A native extension included Linux kernel userspace headers that are not in the runtime.",
            next: &["add linux-headers to robo.nix components", "robo up"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#missing-linux-headers",
        },
        matcher: Matcher::Any(LINUX_HEADERS_MARKERS),
        search_terms: &["linux", "input", "joystick", "headers", "directory"],
    },
    FailureEntry {
        template: DiagnosisTemplate {
            id: "native.python-build-tool-shim",
            title: "Python-owned native build tool shim is crossing the ABI boundary",
            confidence: Confidence::High,
            owner: "Project build invocation plus Python/native boundary",
            summary: "A .venv build-tool executable appears in the failure alongside glibc symbol errors.",
            next: &["robo shell", "which cmake", "which ninja"],
            docs: "https://ausbxuse.github.io/robo-nix/users/failure-guide#native-build-tool-shim-mixing",
        },
        matcher: Matcher::AnyPair {
            required: "GLIBC_",
            one_of: &[".venv/bin/cmake", ".venv/bin/ninja"],
        },
        search_terms: &["venv", "cmake", "ninja", "glibc", "shim"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

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
    fn unknown_text_has_no_match() {
        assert!(ids("some project-specific pytest failure").is_empty());
    }

    #[test]
    fn short_search_phrases_return_possible_matches_not_diagnoses() {
        assert!(ids("EGL display").is_empty());
        assert_eq!(suggestion_ids("EGL display"), vec!["graphics.egl-context"]);
    }

    #[test]
    fn uncertain_output_includes_agent_handoff() {
        let output = diagnose_output("EGL display");

        assert!(output.matches.is_empty());
        assert!(output.agent_handoff.is_some());
    }

    #[test]
    fn confident_output_does_not_include_agent_handoff() {
        let output = diagnose_output("Failed EGL display");

        assert_eq!(output.matches[0].id, "graphics.egl-context");
        assert!(output.agent_handoff.is_none());
    }
}
