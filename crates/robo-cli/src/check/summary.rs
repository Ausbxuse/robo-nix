use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::runtime::{ProjectRuntime, RuntimeWhy};
use crate::{Config, LabelKind, UiSpinner, exact_python_requirement, inline, label};

use super::CheckArgs;
use super::cuda::{CudaCheckPlan, cuda_check_plan};
use super::deep::run_deep_checks;
use super::native::native_tool_wheel_shims;
use super::python::{PythonEnvironmentOrigin, python_environment_origin};

struct Attention {
    title: String,
    details: Vec<String>,
}

impl Attention {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            details: Vec::new(),
        }
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }
}

pub(super) fn run_summary(
    args: CheckArgs,
    config: Config,
    runtime: ProjectRuntime,
    why: RuntimeWhy,
) -> ExitCode {
    let mut progress = UiSpinner::new(config, "checking runtime status");
    let pyproject = fs::read_to_string("pyproject.toml").ok();
    let pyproject_dependencies = pyproject
        .as_deref()
        .map(crate::pyproject::dependency_names)
        .unwrap_or_default();
    let workspace =
        env::current_dir().map_or_else(|_| ".".into(), |path| path.display().to_string());

    let mut issues = 0usize;
    let mut warnings = 0usize;
    let mut project_ready = Vec::new();
    let mut runtime_ready = Vec::new();
    let mut environment_ready = Vec::new();
    let mut attention = Vec::new();

    if Path::new("robo.nix").exists()
        && Path::new("flake.nix").exists()
        && runtime.schema_version.as_deref() == Some("1")
    {
        runtime_ready.push("runtime files".to_string());
    } else {
        warnings += 1;
        attention.push(
            Attention::new("runtime files need review")
                .detail("run: robo init . --force")
                .detail("note: review local edits before regenerating"),
        );
    }

    let python_file = fs::read_to_string(".python-version").ok();
    let python_file_matches = python_file
        .as_deref()
        .map(str::trim)
        .is_some_and(|version| version == runtime.python_version);
    let pyproject_matches = pyproject.as_deref().is_some_and(|pyproject| {
        exact_python_requirement(pyproject)
            .as_deref()
            .is_none_or(|required| required == runtime.python_version)
    });
    if python_file_matches && pyproject_matches {
        project_ready.push("Python contract".to_string());
    } else if pyproject.is_none() {
        warnings += 1;
        attention.push(
            Attention::new("pyproject.toml missing")
                .detail("run: robo init .")
                .detail("note: uv owns Python dependencies"),
        );
    } else {
        issues += 1;
        attention.push(
            Attention::new("Python version mismatch")
                .detail(format!("expected: {}", runtime.python_version))
                .detail("fix: align .python-version, pyproject.toml, and robo.nix"),
        );
    }

    if Path::new("uv.lock").exists() {
        project_ready.push("uv lockfile".to_string());
    } else {
        warnings += 1;
        attention.push(
            Attention::new("uv.lock missing")
                .detail("run: robo shell")
                .detail("     uv sync"),
        );
    }

    match python_environment_origin() {
        PythonEnvironmentOrigin::Missing => {
            warnings += 1;
            attention.push(
                Attention::new("Python environment missing")
                    .detail("run: robo shell")
                    .detail("     uv sync"),
            );
        }
        PythonEnvironmentOrigin::NixBacked(_origin) => {
            environment_ready.push(format!("Nix-backed Python"));
        }
        PythonEnvironmentOrigin::HostBacked(origin) => {
            issues += 1;
            attention.push(
                Attention::new("Python environment was created outside robo-nix")
                    .detail(format!("found: {origin}"))
                    .detail("fix: robo shell -c 'uv venv --python \"$ROBO_NIX_PYTHON\" --clear && uv sync'"),
            );
        }
    }

    let native_tool_shims = native_tool_wheel_shims();
    if !native_tool_shims.is_empty() {
        warnings += 1;
        attention.push(
            Attention::new("Python environment contains native build tool shims")
                .detail(format!("found: {}", native_tool_shims.join(", ")))
                .detail("note: Nix owns CMake, Ninja, compilers, and native build tools"),
        );
    }

    if let Some(pyproject) = pyproject.as_deref() {
        let mut missing = Vec::new();
        for expected in crate::runtime::expected_components_from_pyproject(pyproject) {
            if !runtime
                .components
                .iter()
                .any(|component| component == &expected.name)
            {
                missing.push(expected.name);
            }
        }
        if missing.is_empty() {
            runtime_ready.push("inferred components".to_string());
        } else {
            warnings += missing.len();
            attention.push(
                Attention::new("runtime components may be incomplete")
                    .detail(format!("missing: {}", missing.join(", ")))
                    .detail("run: robo init . --force"),
            );
        }
    }

    let missing_directories: Vec<_> = why
        .required_directories
        .iter()
        .filter(|path| !Path::new(&path.name).is_dir())
        .map(|path| path.name.clone())
        .collect();
    if missing_directories.is_empty() {
        if !why.required_directories.is_empty() {
            runtime_ready.push(format!(
                "required directories ({})",
                why.required_directories.len()
            ));
        }
    } else {
        issues += missing_directories.len();
        attention.push(
            Attention::new("required directories missing")
                .detail(format!("missing: {}", missing_directories.join(", "))),
        );
    }

    summarize_cuda_requirements(
        &runtime,
        args.deep,
        &mut issues,
        &mut warnings,
        &mut environment_ready,
        &mut attention,
    );
    summarize_graphics_environment(&runtime, &mut warnings, &mut attention);
    progress.finish();

    if args.deep {
        if let Err(code) = run_deep_checks(
            config,
            &runtime,
            &pyproject_dependencies,
            &mut issues,
            &mut warnings,
        ) {
            return code;
        }
    }

    print_summary(
        config,
        &runtime,
        &workspace,
        &project_ready,
        &runtime_ready,
        &environment_ready,
        &attention,
        args.deep,
        issues,
        warnings,
    );

    if issues == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn summarize_cuda_requirements(
    runtime: &ProjectRuntime,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
    environment_ready: &mut Vec<String>,
    attention: &mut Vec<Attention>,
) {
    let plan = cuda_check_plan(runtime);
    if !plan.needed() {
        return;
    }

    if plan.host_required {
        summarize_cuda_host_requirement(&plan, issues, warnings, environment_ready, attention);
    }
    if plan.toolkit_required {
        summarize_cuda_toolkit_requirement(&plan, deep, issues, warnings, environment_ready, attention);
    }
}

fn summarize_graphics_environment(
    runtime: &ProjectRuntime,
    warnings: &mut usize,
    attention: &mut Vec<Attention>,
) {
    let Some(mujoco_gl) = forced_mujoco_gl(
        runtime,
        env::var_os("MUJOCO_GL").as_deref(),
        env::var_os("ROBO_NIX_MUJOCO_GL_DEFAULT").as_deref(),
    ) else {
        return;
    };

    *warnings += 1;
    attention.push(
        Attention::new("MuJoCo GL backend is forced")
            .detail(format!("found: MUJOCO_GL={mujoco_gl}"))
            .detail("note: desktop GLFW viewers usually need this unset")
            .detail("fix: unset MUJOCO_GL before running graphical MuJoCo apps"),
    );
}

pub(super) fn forced_mujoco_gl(
    runtime: &ProjectRuntime,
    mujoco_gl: Option<&OsStr>,
    default_mujoco_gl: Option<&OsStr>,
) -> Option<String> {
    if !runtime.components.iter().any(|item| item == "mujoco") {
        return None;
    }
    let value = mujoco_gl?.to_string_lossy();
    if default_mujoco_gl.is_some_and(|default| default.to_string_lossy() == value) {
        return None;
    }
    (!value.is_empty()).then(|| value.into_owned())
}

fn summarize_cuda_host_requirement(
    plan: &CudaCheckPlan,
    issues: &mut usize,
    warnings: &mut usize,
    environment_ready: &mut Vec<String>,
    attention: &mut Vec<Attention>,
) {
    if env::consts::OS != "linux" {
        *issues += 1;
        attention.push(
            Attention::new("CUDA requires a Linux host")
                .detail("fix: use a Linux NVIDIA machine for this runtime"),
        );
        return;
    }

    let Some(host_version) = crate::runtime::host_cuda_driver_version() else {
        *issues += 1;
        attention.push(
            Attention::new("NVIDIA driver stack not found")
                .detail("fix: run on a machine with NVIDIA drivers installed"),
        );
        return;
    };

    if let Some(expected) = plan.expected_wheel_version.as_deref() {
        if crate::runtime::cuda_version_less_than(&host_version, expected) == Some(true) {
            *issues += 1;
            attention.push(
                Attention::new("CUDA host driver is too old")
                    .detail(format!("found: CUDA {host_version}"))
                    .detail(format!("need: CUDA {expected} for uv.lock CUDA wheels"))
                    .detail("fix: upgrade the NVIDIA driver or regenerate uv.lock with older CUDA wheels"),
            );
        } else {
            environment_ready.push(format!("CUDA host driver ({host_version}, needs {expected})"));
        }
    } else {
        environment_ready.push(format!("CUDA host driver ({host_version})"));
    }

    if let Some(path) = crate::runtime::find_host_libcuda() {
        environment_ready.push(format!("CUDA driver library ({path})"));
    } else {
        *warnings += 1;
        attention.push(
            Attention::new("CUDA driver library not found")
                .detail("note: Nix provides the CUDA build toolkit, but libcuda.so.1 comes from the host driver")
                .detail("deep: robo check --deep"),
        );
    }
}

fn summarize_cuda_toolkit_requirement(
    plan: &CudaCheckPlan,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
    environment_ready: &mut Vec<String>,
    attention: &mut Vec<Attention>,
) {
    let Some(cuda_root) = crate::runtime::cuda_root_from_env() else {
        if deep {
            return;
        }
        *warnings += 1;
        let mut item = Attention::new("CUDA toolkit not visible in this shell")
            .detail("run: robo shell")
            .detail("note: CUDA_HOME/CUDA_PATH are set inside the runtime");
        if !deep {
            item = item.detail("deep: robo check --deep validates the Nix CUDA toolkit");
        }
        attention.push(item);
        return;
    };

    let Some(expected) = plan.expected_wheel_version.as_deref() else {
        environment_ready.push(format!("CUDA toolkit ({cuda_root})"));
        return;
    };

    let Some(actual) = crate::runtime::cuda_version_from_root() else {
        *warnings += 1;
        attention.push(
            Attention::new("CUDA toolkit version is unknown")
                .detail(format!("path: {cuda_root}"))
                .detail("deep: robo check --deep"),
        );
        return;
    };

    if actual == expected {
        environment_ready.push(format!("CUDA toolkit ({actual})"));
    } else {
        *issues += 1;
        attention.push(
            Attention::new("CUDA toolkit version does not match uv.lock")
                .detail(format!("found: CUDA {actual} at {cuda_root}"))
                .detail(format!("need: CUDA {expected}")),
        );
    }
}

fn print_summary(
    config: Config,
    runtime: &ProjectRuntime,
    workspace: &str,
    project_ready: &[String],
    runtime_ready: &[String],
    environment_ready: &[String],
    attention: &[Attention],
    deep: bool,
    issues: usize,
    warnings: usize,
) {
    let status = if issues == 0 { "ok" } else { "error" };
    let status_kind = if issues == 0 {
        LabelKind::Ok
    } else {
        LabelKind::Error
    };

    println!(
        "{}  {}  python={}  {}{}",
        runtime.env_name,
        label(config, status, status_kind),
        runtime.python_version,
        count_label(config, warnings, "warning", LabelKind::Warn),
        if issues == 0 {
            String::new()
        } else {
            format!(
                "  {}",
                count_label(config, issues, "issue", LabelKind::Error)
            )
        }
    );
    println!("{}", workspace);

    if !project_ready.is_empty() {
        print_compact_row(config, LabelKind::Ok, "project", &project_ready.join(", "));
    }

    if !runtime_ready.is_empty() {
        print_compact_row(config, LabelKind::Ok, "runtime", &runtime_ready.join(", "));
    }

    if !environment_ready.is_empty() {
        print_compact_row(
            config,
            LabelKind::Ok,
            "environment",
            &environment_ready.join(", "),
        );
    }

    for item in attention {
        print_compact_attention(config, item);
    }

    if !deep {
        print_compact_command_row(
            config,
            LabelKind::Warn,
            "skipped",
            "deep runtime probes",
            "robo check --deep",
        );
    }
}

fn count_label(config: Config, count: usize, noun: &str, kind: LabelKind) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!("{} {noun}{suffix}", label(config, &count.to_string(), kind))
}

fn print_compact_row(config: Config, kind: LabelKind, label_text: &str, body: &str) {
    println!(
        "{} {}: {}",
        label(config, "✓", kind),
        label(config, &format!("{label_text}:"), LabelKind::Status),
        inline(config, body),
    );
}

fn print_compact_command_row(
    config: Config,
    kind: LabelKind,
    label_text: &str,
    body: &str,
    command: &str,
) {
    print!(
        "{} {}: {}: ",
        label(config, "!", kind),
        label(config, &format!("{label_text}:"), LabelKind::Status),
        inline(config, body),
    );
    println!("{}", label(config, command, LabelKind::Command));
}

fn print_compact_attention(config: Config, item: &Attention) {
    let label_text = compact_attention_label(&item.title);
    let body = compact_attention_body(item);
    println!(
        "{} {}: {}",
        label(config, "!", LabelKind::Warn),
        label(config, &format!("{label_text}:"), LabelKind::Status),
        inline(config, &body),
    );
    for detail in &item.details {
        println!("  {}", summary_detail(config, detail));
    }
}

fn compact_attention_label(title: &str) -> &str {
    if title.starts_with("Python version mismatch")
        || title.starts_with("pyproject.toml missing")
        || title.starts_with("uv.lock missing")
    {
        "project"
    } else if title.starts_with("runtime files")
        || title.starts_with("runtime components")
        || title.starts_with("required directories")
    {
        "runtime"
    } else {
        "environment"
    }
}

fn compact_attention_body(item: &Attention) -> String {
    if item.title == "Python environment contains native build tool shims" {
        if let Some(found) = item.details.iter().find_map(|detail| detail.strip_prefix("found: ")) {
            return format!("native build tool shims: {found}");
        }
    }

    item.title.clone()
}

fn summary_detail(config: Config, detail: &str) -> String {
    if let Some(command) = detail.strip_prefix("run: ") {
        format!(
            "{} {}",
            label(config, "run:", LabelKind::Hint),
            label(config, command, LabelKind::Command)
        )
    } else if let Some(command) = detail.strip_prefix("     ") {
        format!("     {}", label(config, command, LabelKind::Command))
    } else if let Some(note) = detail.strip_prefix("note: ") {
        format!("{} {}", label(config, "note:", LabelKind::Hint), note)
    } else if let Some(command) = detail.strip_prefix("deep: ") {
        format!(
            "{} {}",
            label(config, "deep:", LabelKind::Hint),
            label(config, command, LabelKind::Command)
        )
    } else {
        inline(config, detail)
    }
}
