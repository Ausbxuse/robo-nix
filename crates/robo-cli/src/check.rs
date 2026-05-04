use clap::{Args, ValueEnum};
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::runtime::{
    ProjectRuntime, RuntimeWhy, WhyEntry, build_runtime_why, read_project_runtime,
};
use crate::{
    Config, LabelKind, UiProgress, UiSpinner, add_runtime_source_override, combined_output,
    command_for_runtime, ensure_project_runtime, error, exact_python_requirement, field, inline,
    label, nix_command, quoted_value, run_bootstrap_with_progress, section,
};

mod egl;

#[derive(Args)]
pub(crate) struct CheckArgs {
    #[arg(value_enum, help = "Focused check domain")]
    domain: Option<CheckDomain>,

    #[arg(long, help = "Run runtime probes that may realize larger Nix closures")]
    deep: bool,

    #[arg(long, help = "Show evidence behind the check result")]
    verbose: bool,

    #[arg(long, help = "Explain why runtime entries are present")]
    why: bool,

    #[arg(long, requires = "why", help = "Emit machine-readable provenance")]
    json: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CheckDomain {
    Graphics,
    Native,
    Python,
    Cuda,
    Ros,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixDryRunItem {
    drv_path: String,
    outputs: std::collections::BTreeMap<String, String>,
}

const NATIVE_TOOL_WHEEL_PACKAGES: &[&str] = &["cmake", "ninja", "patchelf"];

pub(crate) fn run(args: CheckArgs, config: Config) -> ExitCode {
    run_with_mode(args, config, CheckMode::Doctor)
}

pub(crate) fn run_check(args: CheckArgs, config: Config) -> ExitCode {
    run_with_mode(args, config, CheckMode::Check)
}

pub(crate) fn run_status(config: Config) -> ExitCode {
    run_with_mode(CheckArgs::default(), config, CheckMode::Status)
}

enum CheckMode {
    Check,
    Doctor,
    Status,
}

impl Default for CheckArgs {
    fn default() -> Self {
        Self {
            domain: None,
            deep: false,
            verbose: false,
            why: false,
            json: false,
        }
    }
}

fn run_with_mode(args: CheckArgs, config: Config, mode: CheckMode) -> ExitCode {
    let mut preflight = (!args.json).then(|| UiSpinner::new(config, "loading runtime contract"));
    if let Err(code) = ensure_project_runtime(config) {
        if let Some(progress) = &mut preflight {
            progress.finish();
        }
        return code;
    }

    let runtime = read_project_runtime();
    let why = build_runtime_why(&runtime);
    if let Some(progress) = &mut preflight {
        progress.finish();
    }

    if args.why && args.json {
        match serde_json::to_string_pretty(&why) {
            Ok(text) => {
                println!("{text}");
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                error(
                    config,
                    &format!("failed to encode runtime provenance: {err}"),
                );
                return ExitCode::from(1);
            }
        }
    }

    if matches!(mode, CheckMode::Status) {
        return run_summary(args, config, runtime, why);
    }

    if matches!(mode, CheckMode::Doctor)
        && args.domain.is_none()
        && !args.deep
        && !args.verbose
        && !args.why
    {
        return run_summary(args, config, runtime, why);
    }

    if matches!(mode, CheckMode::Check) {
        return run_check_surface(args, config, runtime, why);
    }

    run_detailed(args, config, runtime, why)
}

fn run_check_surface(
    args: CheckArgs,
    config: Config,
    runtime: ProjectRuntime,
    why: RuntimeWhy,
) -> ExitCode {
    if args.why {
        if args.json {
            match serde_json::to_string_pretty(&why) {
                Ok(text) => {
                    println!("{text}");
                    return ExitCode::SUCCESS;
                }
                Err(err) => {
                    error(
                        config,
                        &format!("failed to encode runtime provenance: {err}"),
                    );
                    return ExitCode::from(1);
                }
            }
        }
        print_runtime_why(config, &why);
        return ExitCode::SUCCESS;
    }

    if args.json {
        error(config, "`robo check --json` is not implemented yet; use `robo check --why --json` for runtime provenance");
        return ExitCode::from(2);
    }

    match args.domain {
        Some(CheckDomain::Graphics) => run_graphics_check(config, &runtime, args.verbose),
        Some(CheckDomain::Native) => run_native_check(config, &runtime, args.verbose),
        Some(CheckDomain::Python) => run_python_check(config, &runtime),
        Some(CheckDomain::Cuda) => run_cuda_check(config, &runtime, args.deep),
        Some(CheckDomain::Ros) => run_ros_check(config, &runtime),
        None if args.deep || args.verbose => run_detailed(args, config, runtime, why),
        None => run_default_check(config, &runtime),
    }
}

fn run_default_check(config: Config, runtime: &ProjectRuntime) -> ExitCode {
    let pyproject = fs::read_to_string("pyproject.toml").ok();
    let missing_components = pyproject
        .as_deref()
        .map(|text| {
            crate::runtime::expected_components_from_pyproject(text)
                .into_iter()
                .filter(|expected| !runtime_has_component(runtime, &expected.name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if runtime.schema_version.as_deref() != Some("1") {
        println!("robo cannot trust this runtime contract yet.\n");
        println!("The generated robo.nix schema is missing or newer than this CLI understands.");
        println!("Review local edits, then regenerate the runtime files:");
        print_command(config, "robo init . --force");
        return ExitCode::from(1);
    }

    if !missing_components.is_empty() {
        println!("robo may be missing runtime components for {}.\n", runtime.env_name);
        for component in &missing_components {
            println!("- {}: {}", component.name, component.reason);
        }
        println!();
        println!("Review robo.nix, then refresh the runtime:");
        print_command(config, "robo up");
        return ExitCode::from(1);
    }

    match python_environment_origin() {
        PythonEnvironmentOrigin::HostBacked(origin) => {
            println!("robo is blocked for {}.\n", runtime.env_name);
            println!("The Python environment was created outside the robo runtime.");
            println!("Found interpreter origin: {origin}");
            println!();
            println!("Recreate it inside the runtime:");
            print_command(config, "robo shell");
            print_command(config, "uv venv --python \"$ROBO_NIX_PYTHON\" --clear");
            print_command(config, "uv sync");
            return ExitCode::from(1);
        }
        PythonEnvironmentOrigin::Missing => {
            println!(
                "{} {}\n",
                label(config, "ready:", LabelKind::Ok),
                inline(config, &format!("runtime is prepared for `{}`", runtime.env_name))
            );
            println!(
                "{} {}",
                label(config, "python:", LabelKind::Warn),
                inline(config, "packages are not synced yet")
            );
            println!(
                "{} {}",
                label(config, "owner:", LabelKind::Status),
                inline(config, "uv owns package sync; use the command documented by this project")
            );
            println!(
                "{} {}",
                label(config, "default:", LabelKind::Hint),
                label(config, "uv sync", LabelKind::Command)
            );
            return ExitCode::SUCCESS;
        }
        PythonEnvironmentOrigin::NixBacked(_) => {}
    }

    let tools = native_tool_wheel_shims();
    if !tools.is_empty() {
        println!("robo found Python-owned native build tool shims.\n");
        println!("Found: {}", tools.join(", "));
        println!("Nix should own CMake, Ninja, compilers, and native build tools.");
        println!();
        println!("If native builds fail, run:");
        print_command(config, "robo check native --verbose");
        return ExitCode::SUCCESS;
    }

    println!(
        "{} {}\n",
        label(config, "ready:", LabelKind::Ok),
        inline(config, &format!("runtime is prepared for `{}`", runtime.env_name))
    );
    println!(
        "{} {}",
        label(config, "runtime:", LabelKind::Status),
        inline(config, "Nix runtime, uv Python environment, and inferred components are aligned")
    );
    println!(
        "{} {}",
        label(config, "diagnostics:", LabelKind::Status),
        inline(config, "run a focused check when a runtime area fails")
    );
    print_command(config, "robo check graphics");
    print_command(config, "robo check native");
    if cuda_check_plan(runtime).needed() {
        print_command(config, "robo check cuda");
    }
    print_command(config, "robo check --deep");
    ExitCode::SUCCESS
}

fn run_graphics_check(config: Config, runtime: &ProjectRuntime, verbose: bool) -> ExitCode {
    if !graphics_relevant(runtime) {
        println!("Graphics runtime is not required by this project.");
        return ExitCode::SUCCESS;
    }

    if let Some(mujoco_gl) = forced_mujoco_gl(runtime, env::var_os("MUJOCO_GL").as_deref()) {
        println!("Graphics runtime is blocked by MUJOCO_GL.\n");
        println!("Found MUJOCO_GL={mujoco_gl}.");
        println!("Desktop GLFW viewers usually need this unset.");
        println!();
        println!("Try:");
        print_command(config, "unset MUJOCO_GL");
        print_command(config, "robo check graphics");
        return ExitCode::from(1);
    }

    if !runtime_has_component(runtime, "x11-gl") {
        println!("Graphics runtime is not selected for this project.\n");
        println!("This runtime appears to need desktop graphics, but robo.nix does not include `x11-gl`.");
        println!();
        println!("Add `x11-gl` to components in robo.nix, then run:");
        print_command(config, "robo up");
        return ExitCode::from(1);
    }

    let output = runtime_output_with_spinner(
        config,
        "graphics: probing EGL and display runtime",
        "bash",
        ["-lc", egl::PROBE_SCRIPT],
        [],
    );
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            println!("Graphics runtime could not be probed.\n");
            println!("{}", combined_output(&output));
            println!("Run with more evidence:");
            print_command(config, "robo check graphics --verbose");
            return ExitCode::from(1);
        }
        Err(err) => {
            println!("Graphics runtime could not be probed.\n");
            println!("Failed to start the runtime probe: {err}");
            return ExitCode::from(1);
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let findings = egl::findings(&egl::parse(&text));
    let errors = findings
        .iter()
        .filter(|finding| finding.kind == egl::FindingKind::Error)
        .collect::<Vec<_>>();
    let warnings = findings
        .iter()
        .filter(|finding| finding.kind == egl::FindingKind::Warn)
        .collect::<Vec<_>>();

    if let Some(finding) = errors.first() {
        println!("Graphics runtime is blocked.\n");
        println!("{}", finding.message);
        if let Some(hint) = finding.hint {
            println!();
            println!("{hint}");
        }
        println!();
        println!("Show full evidence:");
        print_command(config, "robo check graphics --verbose");
        return ExitCode::from(1);
    }

    if let Some(finding) = warnings.first() {
        println!("Graphics runtime needs attention.\n");
        println!("{}", finding.message);
        if let Some(hint) = finding.hint {
            println!();
            println!("{hint}");
        }
        if verbose {
            print_graphics_evidence(config, &findings);
        }
        return ExitCode::SUCCESS;
    }

    println!("Graphics runtime looks ready.\n");
    for finding in findings
        .iter()
        .filter(|finding| finding.kind == egl::FindingKind::Ok)
        .take(2)
    {
        println!("Found {}", finding.message);
    }
    if verbose {
        print_graphics_evidence(config, &findings);
    }
    ExitCode::SUCCESS
}

fn run_native_check(config: Config, runtime: &ProjectRuntime, verbose: bool) -> ExitCode {
    let tools = native_tool_wheel_shims();
    if !native_relevant(runtime) && tools.is_empty() {
        println!("Native build support is not required by this project.");
        return ExitCode::SUCCESS;
    }

    if !runtime_has_component(runtime, "native-build") {
        println!("Native build support is not selected for this project.\n");
        println!("This project appears to need C/C++ build tooling, but robo.nix does not include `native-build`.");
        println!();
        println!("Add `native-build` to components in robo.nix, then run:");
        print_command(config, "robo up");
        return ExitCode::from(1);
    }

    if !tools.is_empty() {
        println!("Native build support needs attention.\n");
        println!("The Python environment contains native build tool shims: {}.", tools.join(", "));
        println!("Nix should own CMake, Ninja, compilers, and native build tools.");
        println!();
        println!("If builds call .venv/bin tools, prefer the runtime tools instead.");
        return ExitCode::SUCCESS;
    }

    let script = r#"
missing=0
for tool in cc c++ cmake pkg-config; do
  if command -v "$tool" >/dev/null 2>&1; then
    printf 'found %s=%s\n' "$tool" "$(command -v "$tool")"
  else
    printf 'missing %s\n' "$tool"
    missing=1
  fi
done
exit "$missing"
"#;

    match runtime_output_with_spinner(
        config,
        "native: probing compiler and build tools",
        "bash",
        ["-lc", script],
        [],
    ) {
        Ok(output) if output.status.success() => {
            println!("Native build support looks ready.\n");
            println!("Found a compiler, CMake, pkg-config, and common C/C++ build tooling.");
            if verbose {
                println!();
                section(config, "evidence");
                print!("{}", combined_output(&output));
            }
            ExitCode::SUCCESS
        }
        Ok(output) => {
            println!("Native build support is incomplete.\n");
            println!("{}", combined_output(&output));
            println!("Nix owns these native build tools. Review the `native-build` component, then run:");
            print_command(config, "robo up");
            ExitCode::from(1)
        }
        Err(err) => {
            println!("Native build support could not be probed.\n");
            println!("Failed to start the runtime probe: {err}");
            ExitCode::from(1)
        }
    }
}

fn run_python_check(config: Config, runtime: &ProjectRuntime) -> ExitCode {
    match python_environment_origin() {
        PythonEnvironmentOrigin::Missing => {
            println!("Python packages are not synced yet.\n");
            println!("Robo found the runtime contract for Python {}, but no `.venv` exists.", runtime.python_version);
            println!("Run the uv command documented by this project.");
            println!("Default: `{}`", label(config, "uv sync", LabelKind::Command));
            ExitCode::SUCCESS
        }
        PythonEnvironmentOrigin::NixBacked(origin) => {
            println!("Python environment looks aligned.\n");
            println!("Found robo runtime Python: {origin}");
            ExitCode::SUCCESS
        }
        PythonEnvironmentOrigin::HostBacked(origin) => {
            println!("Python environment is blocked.\n");
            println!("The active `.venv` was created outside the robo runtime.");
            println!("Found interpreter origin: {origin}");
            println!();
            println!("Recreate it inside the runtime:");
            print_command(config, "robo shell");
            print_command(config, "uv venv --python \"$ROBO_NIX_PYTHON\" --clear");
            print_command(config, "uv sync");
            ExitCode::from(1)
        }
    }
}

fn run_cuda_check(config: Config, runtime: &ProjectRuntime, deep: bool) -> ExitCode {
    let plan = cuda_check_plan(runtime);
    if !plan.needed() {
        println!("CUDA runtime is not required by this project.");
        return ExitCode::SUCCESS;
    }

    let mut issues = 0;
    let mut warnings = 0;
    check_cuda_host(config, runtime, deep, &mut issues, &mut warnings);
    if issues == 0 {
        println!();
        println!("CUDA checks finished without blocking issues.");
        ExitCode::SUCCESS
    } else {
        println!();
        println!("CUDA checks found blocking issues.");
        ExitCode::from(1)
    }
}

fn run_ros_check(_config: Config, runtime: &ProjectRuntime) -> ExitCode {
    if runtime_has_component(runtime, "ros2-jazzy") || runtime_has_component(runtime, "ros-workspace") {
        println!("ROS runtime is selected for this project.\n");
        println!("Focused ROS probes are not implemented yet.");
        println!("Use `robo check --deep` for the current broad runtime validation.");
    } else {
        println!("ROS runtime is not required by this project.");
    }
    ExitCode::SUCCESS
}

fn graphics_relevant(runtime: &ProjectRuntime) -> bool {
    ["x11-gl", "mujoco", "qt6", "matplotlib-qt", "isaac-sim"]
        .iter()
        .any(|component| runtime_has_component(runtime, component))
}

fn native_relevant(runtime: &ProjectRuntime) -> bool {
    ["native-build", "mujoco", "qt6", "cuda-toolkit", "isaac-sim", "ros2-jazzy"]
        .iter()
        .any(|component| runtime_has_component(runtime, component))
}

fn print_command(config: Config, command: &str) {
    println!("  {}", label(config, command, LabelKind::Command));
}

fn print_graphics_evidence(config: Config, findings: &[egl::Finding]) {
    println!();
    section(config, "evidence");
    for finding in findings {
        let label_text = match finding.kind {
            egl::FindingKind::Ok => "ok:",
            egl::FindingKind::Warn => "warn:",
            egl::FindingKind::Error => "error:",
        };
        let label_kind = match finding.kind {
            egl::FindingKind::Ok => LabelKind::Ok,
            egl::FindingKind::Warn => LabelKind::Warn,
            egl::FindingKind::Error => LabelKind::Error,
        };
        check_line(config, label_text, label_kind, &finding.message);
        if let Some(hint) = finding.hint {
            check_hint(config, hint);
        }
    }
}

fn run_detailed(
    args: CheckArgs,
    config: Config,
    runtime: ProjectRuntime,
    why: RuntimeWhy,
) -> ExitCode {
    let pyproject = fs::read_to_string("pyproject.toml").ok();
    let pyproject_dependencies = pyproject
        .as_deref()
        .map(crate::pyproject::dependency_names)
        .unwrap_or_default();
    let mut issues = 0;
    let mut warnings = 0;

    check_field(config, &format!("env={}", runtime.env_name));
    check_field(config, &format!("python={}", runtime.python_version));
    check_field(
        config,
        &format!(
            "workspace={}",
            env::current_dir().map_or_else(|_| ".".into(), |path| path.display().to_string())
        ),
    );

    check_ok(config, "workspace root exists");
    check_schema_version(config, &runtime, &mut warnings);
    check_lock_freshness(config, &mut warnings);
    check_python_files(
        config,
        &runtime,
        pyproject.as_deref(),
        &mut issues,
        &mut warnings,
    );
    check_uv_files(config, &mut warnings);
    check_python_environment(config, &mut issues, &mut warnings);
    check_native_tool_wheel_shims(config, &mut warnings);
    check_expected_components(config, &runtime, pyproject.as_deref(), &mut warnings);
    check_graphics_environment(config, &runtime, &mut warnings);
    check_required_paths(config, &why, &mut issues);
    check_cuda_host(config, &runtime, args.deep, &mut issues, &mut warnings);
    check_suggestions(config, &runtime);
    if args.why {
        print_runtime_why(config, &why);
    }

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
    } else {
        check_hint(config, "deep runtime probes skipped");
    }

    if issues == 0 {
        if args.deep {
            check_next(
                config,
                "run 'robo dry-run' if you want a bootstrap-only validation pass",
            );
        } else {
            check_next(
                config,
                "run 'robo check --deep' before debugging native runtime failures",
            );
        }
        check_next(config, "run 'robo shell' to enter the environment");
        check_status(config, "ok", LabelKind::Ok, 0, warnings);
        ExitCode::SUCCESS
    } else {
        check_next(config, "fix the issues above and rerun 'robo check --deep'");
        check_status(config, "error", LabelKind::Error, issues, warnings);
        ExitCode::from(1)
    }
}

fn print_runtime_why(config: Config, why: &RuntimeWhy) {
    check_why(
        config,
        &format!(
            "profile {}",
            why.profile.as_deref().unwrap_or("manual/unknown")
        ),
    );
    print_why_group(config, "components", &why.components);
    print_why_group(config, "required directories", &why.required_directories);
    print_why_group(config, "required files", &why.required_files);
    print_why_group(config, "bootstrap scripts", &why.bootstrap_scripts);
    print_why_group(config, "suggestions", &why.suggestions);
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct CudaCheckPlan {
    expected_wheel_version: Option<String>,
    host_required: bool,
    toolkit_required: bool,
}

impl CudaCheckPlan {
    fn needed(&self) -> bool {
        self.host_required || self.toolkit_required
    }
}

fn cuda_check_plan(runtime: &ProjectRuntime) -> CudaCheckPlan {
    let expected_wheel_version = runtime
        .cuda_wheel_version
        .clone()
        .or_else(crate::runtime::infer_cuda_wheel_version_from_uv_lock);
    cuda_check_plan_from_expected(runtime, expected_wheel_version)
}

fn cuda_check_plan_from_expected(
    runtime: &ProjectRuntime,
    expected_wheel_version: Option<String>,
) -> CudaCheckPlan {
    let toolkit_required = runtime_has_component(runtime, "cuda-toolkit");
    let host_required =
        expected_wheel_version.is_some() || runtime_has_component(runtime, "isaac-sim");
    CudaCheckPlan {
        expected_wheel_version,
        host_required,
        toolkit_required,
    }
}

fn runtime_has_component(runtime: &ProjectRuntime, component: &str) -> bool {
    runtime.components.iter().any(|item| item == component)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PythonEnvironmentOrigin {
    Missing,
    NixBacked(String),
    HostBacked(String),
}

fn python_environment_origin() -> PythonEnvironmentOrigin {
    if !Path::new(".venv").is_dir() {
        return PythonEnvironmentOrigin::Missing;
    }

    let origin = fs::read_to_string(".venv/pyvenv.cfg")
        .ok()
        .and_then(|config| {
            config.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name.trim() == "home").then(|| value.trim().to_string())
            })
        })
        .or_else(|| {
            fs::canonicalize(".venv/bin/python")
                .ok()
                .map(|path| path.display().to_string())
        });

    match origin {
        Some(path) if path.starts_with("/nix/store/") => PythonEnvironmentOrigin::NixBacked(path),
        Some(path) => PythonEnvironmentOrigin::HostBacked(path),
        None => PythonEnvironmentOrigin::HostBacked(".venv/bin/python".to_string()),
    }
}

fn run_summary(
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
    let mut ready = Vec::new();
    let mut attention = Vec::new();

    if Path::new("robo.nix").exists()
        && Path::new("flake.nix").exists()
        && runtime.schema_version.as_deref() == Some("1")
    {
        ready.push("runtime files".to_string());
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
        ready.push("Python contract".to_string());
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
        ready.push("uv lockfile".to_string());
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
        PythonEnvironmentOrigin::NixBacked(origin) => {
            ready.push(format!("Python environment ({origin})"));
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
            ready.push("inferred components".to_string());
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
            ready.push(format!(
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
        &mut ready,
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
        config, &runtime, &workspace, &ready, &attention, args.deep, issues, warnings,
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
    ready: &mut Vec<String>,
    attention: &mut Vec<Attention>,
) {
    let plan = cuda_check_plan(runtime);
    if !plan.needed() {
        return;
    }

    if plan.host_required {
        summarize_cuda_host_requirement(&plan, issues, warnings, ready, attention);
    }
    if plan.toolkit_required {
        summarize_cuda_toolkit_requirement(&plan, deep, issues, warnings, ready, attention);
    }
}

fn summarize_graphics_environment(
    runtime: &ProjectRuntime,
    warnings: &mut usize,
    attention: &mut Vec<Attention>,
) {
    let Some(mujoco_gl) = forced_mujoco_gl(runtime, env::var_os("MUJOCO_GL").as_deref()) else {
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

fn check_graphics_environment(config: Config, runtime: &ProjectRuntime, warnings: &mut usize) {
    let Some(mujoco_gl) = forced_mujoco_gl(runtime, env::var_os("MUJOCO_GL").as_deref()) else {
        return;
    };

    check_warn(
        config,
        warnings,
        &format!("MuJoCo GL backend forced by MUJOCO_GL={mujoco_gl}"),
    );
    check_hint(
        config,
        "desktop GLFW viewers usually need MUJOCO_GL unset; headless probes may set it narrowly",
    );
}

fn forced_mujoco_gl(runtime: &ProjectRuntime, mujoco_gl: Option<&OsStr>) -> Option<String> {
    if !runtime_has_component(runtime, "mujoco") {
        return None;
    }
    let value = mujoco_gl?.to_string_lossy();
    (!value.is_empty()).then(|| value.into_owned())
}

fn summarize_cuda_host_requirement(
    plan: &CudaCheckPlan,
    issues: &mut usize,
    warnings: &mut usize,
    ready: &mut Vec<String>,
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
            ready.push(format!("CUDA host driver ({host_version}, needs {expected})"));
        }
    } else {
        ready.push(format!("CUDA host driver ({host_version})"));
    }

    if let Some(path) = crate::runtime::find_host_libcuda() {
        ready.push(format!("CUDA driver library ({path})"));
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
    ready: &mut Vec<String>,
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
        ready.push(format!("CUDA toolkit ({cuda_root})"));
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
        ready.push(format!("CUDA toolkit ({actual})"));
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
    ready: &[String],
    attention: &[Attention],
    deep: bool,
    issues: usize,
    warnings: usize,
) {
    println!(
        "{} {}\n",
        label(config, "checked", LabelKind::Status),
        runtime.env_name
    );

    section(config, "project");
    field(config, "python", &runtime.python_version);
    field(config, "workspace", workspace);

    if !ready.is_empty() {
        println!();
        section(config, "ready");
        for item in ready {
            println!("  {} {}", label(config, "✓", LabelKind::Ok), item);
        }
    }

    if !attention.is_empty() {
        println!();
        section(config, "attention");
        for item in attention {
            println!("  {} {}", label(config, "!", LabelKind::Warn), item.title);
            for detail in &item.details {
                println!("    {}", summary_detail(config, detail));
            }
            println!();
        }
    }

    if !deep {
        section(config, "skipped");
        println!("  deep runtime probes");
        println!("    {}", summary_detail(config, "run: robo check --deep"));
        println!();
    }

    section(config, "status");
    let status_kind = if issues == 0 {
        LabelKind::Ok
    } else {
        LabelKind::Error
    };
    let status = if issues == 0 { "ok" } else { "error" };
    println!(
        "  {}, {}{}",
        label(config, status, status_kind),
        count_label(config, warnings, "warning", LabelKind::Warn),
        if issues == 0 {
            String::new()
        } else {
            format!(
                ", {}",
                count_label(config, issues, "issue", LabelKind::Error)
            )
        }
    );
}

fn count_label(config: Config, count: usize, noun: &str, kind: LabelKind) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!("{} {noun}{suffix}", label(config, &count.to_string(), kind))
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

fn print_why_group(config: Config, title: &str, entries: &[WhyEntry]) {
    if entries.is_empty() {
        return;
    }
    check_why(config, title);
    for entry in entries {
        check_why(
            config,
            &format!("  {} <- {}: {}", entry.name, entry.source, entry.reason),
        );
    }
}

fn check_python_files(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject: Option<&str>,
    issues: &mut usize,
    warnings: &mut usize,
) {
    match fs::read_to_string(".python-version") {
        Ok(version) => {
            let version = version.trim();
            if version == runtime.python_version {
                check_ok(
                    config,
                    &format!(".python-version matches {}", runtime.python_version),
                );
            } else {
                check_warn(
                    config,
                    warnings,
                    &format!(
                        ".python-version is {version} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(
                    config,
                    "update .python-version or pythonVersion in robo.nix",
                );
            }
        }
        Err(_) => {
            check_warn(config, warnings, ".python-version is missing");
            check_hint(
                config,
                &format!(
                    "create .python-version with {} so uv uses the intended interpreter",
                    runtime.python_version
                ),
            );
        }
    }

    if let Some(pyproject) = pyproject {
        if let Some(required) = exact_python_requirement(pyproject) {
            if required == runtime.python_version {
                check_ok(
                    config,
                    &format!("pyproject.toml requires Python {required}"),
                );
            } else {
                check_error(
                    config,
                    issues,
                    &format!(
                        "pyproject.toml requires Python {required} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(
                    config,
                    &format!(
                        "set `pythonVersion = \"{required}\";` in robo.nix and write `{required}` to .python-version"
                    ),
                );
            }
        }
    } else {
        check_warn(config, warnings, "pyproject.toml is missing");
        check_hint(config, "run `robo init .` or create pyproject.toml for uv");
    }
}

fn check_python_environment(config: Config, issues: &mut usize, warnings: &mut usize) {
    match python_environment_origin() {
        PythonEnvironmentOrigin::Missing => {
            check_warn(config, warnings, "Python virtualenv is missing");
            check_hint(config, "run `robo shell`, then `uv sync`");
        }
        PythonEnvironmentOrigin::NixBacked(origin) => {
            check_ok(
                config,
                &format!("Python virtualenv uses robo-nix Python ({origin})"),
            );
        }
        PythonEnvironmentOrigin::HostBacked(origin) => {
            check_error(
                config,
                issues,
                "Python virtualenv was created outside robo-nix",
            );
            check_hint(config, &format!("found interpreter origin: {origin}"));
            check_hint(
                config,
                "run `robo shell -c 'uv venv --python \"$ROBO_NIX_PYTHON\" --clear && uv sync'`",
            );
            check_hint(
                config,
                "Nix runtime libraries require an ABI-aligned Python process on older distros",
            );
        }
    }
}

fn check_native_tool_wheel_shims(config: Config, warnings: &mut usize) {
    let tools = native_tool_wheel_shims();
    if tools.is_empty() {
        return;
    }

    check_warn(
        config,
        warnings,
        &format!(
            "Python virtualenv contains native build tool shims: {}",
            tools.join(", ")
        ),
    );
    check_hint(
        config,
        "uv owns Python packages; Nix owns CMake, Ninja, compilers, and native build tools",
    );
    check_hint(
        config,
        "project bootstrap scripts should call native build tools from the runtime, not .venv/bin shims",
    );
}

fn native_tool_wheel_shims() -> Vec<String> {
    let package_names = fs::read_to_string("uv.lock")
        .ok()
        .map(|lock| uv_lock_package_names(&lock))
        .unwrap_or_default();

    NATIVE_TOOL_WHEEL_PACKAGES
        .iter()
        .filter(|name| package_names.contains(**name))
        .filter(|name| Path::new(".venv/bin").join(name).exists())
        .map(|name| (*name).to_string())
        .collect()
}

fn uv_lock_package_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_package = false;
    let mut package_name_seen = false;

    for line in text.lines().map(str::trim) {
        if line == "[[package]]" {
            in_package = true;
            package_name_seen = false;
            continue;
        }

        if line.starts_with('[') {
            in_package = false;
            continue;
        }

        if in_package
            && !package_name_seen
            && line.starts_with("name")
            && let Some(name) = quoted_value(line)
        {
            names.insert(normalize_package_name(name));
            package_name_seen = true;
        }
    }

    names
}

fn normalize_package_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '_' | '.' => '-',
            _ => ch.to_ascii_lowercase(),
        })
        .collect()
}

fn check_schema_version(config: Config, runtime: &ProjectRuntime, warnings: &mut usize) {
    match runtime.schema_version.as_deref() {
        Some("1") => check_ok(config, "robo.nix schema version is 1"),
        Some(version) => {
            check_warn(
                config,
                warnings,
                &format!("robo.nix schema version {version} is newer than this robo supports"),
            );
            check_hint(
                config,
                "upgrade robo-nix or regenerate with `robo init . --force` after reviewing local edits",
            );
        }
        None => {
            check_warn(config, warnings, "robo.nix schema version is missing");
            check_hint(
                config,
                "rerun `robo init . --force` when you are ready to migrate this generated file",
            );
        }
    }
}

fn check_uv_files(config: Config, warnings: &mut usize) {
    if Path::new("uv.lock").exists() {
        check_ok(config, "uv.lock is present");
    } else {
        check_warn(config, warnings, "uv.lock is missing");
        check_hint(
            config,
            "run 'robo shell', then run 'uv sync' after defining pyproject.toml dependencies",
        );
    }

    if Path::new(".venv").is_dir() {
        check_ok(config, "uv virtual environment exists");
    } else {
        check_warn(config, warnings, "uv virtual environment is missing");
        check_hint(
            config,
            "run 'robo shell', then run 'uv sync' to create .venv",
        );
    }
}

fn check_expected_components(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject: Option<&str>,
    warnings: &mut usize,
) {
    let Some(pyproject) = pyproject else {
        return;
    };
    let mut matched = Vec::new();
    for expected in crate::runtime::expected_components_from_pyproject(pyproject) {
        if runtime
            .components
            .iter()
            .any(|component| component == &expected.name)
        {
            matched.push(expected.name);
        } else {
            check_warn(
                config,
                warnings,
                &format!(
                    "pyproject.toml implies component {} but robo.nix does not select it",
                    expected.name
                ),
            );
            check_hint(
                config,
                &format!(
                    "reason: {}; rerun `robo init . --force` or add `{}` to components",
                    expected.reason, expected.name
                ),
            );
        }
    }
    if !matched.is_empty() {
        check_ok(
            config,
            &format!(
                "pyproject runtime expectations satisfied ({})",
                matched.join(", ")
            ),
        );
    }
}

fn check_required_paths(config: Config, why: &RuntimeWhy, issues: &mut usize) {
    let mut directory_count = 0usize;
    for path in &why.required_directories {
        if Path::new(&path.name).is_dir() {
            directory_count += 1;
        } else {
            check_error(
                config,
                issues,
                &format!("required directory is missing: {}", path.name),
            );
            check_hint(config, &path.remediation_hint);
        }
    }
    if directory_count > 0 {
        check_ok(
            config,
            &format!("required directories exist ({directory_count})"),
        );
    }

    let mut file_count = 0usize;
    for path in &why.required_files {
        if Path::new(&path.name).is_file() {
            file_count += 1;
        } else {
            check_error(
                config,
                issues,
                &format!("required file is missing: {}", path.name),
            );
            check_hint(config, &path.remediation_hint);
        }
    }
    if file_count > 0 {
        check_ok(config, &format!("required files exist ({file_count})"));
    }

    let mut script_count = 0usize;
    for script in &why.bootstrap_scripts {
        if Path::new(&script.name).is_file() {
            script_count += 1;
        } else {
            check_error(
                config,
                issues,
                &format!("bootstrap script is missing: {}", script.name),
            );
            check_hint(config, &script.remediation_hint);
        }
    }
    if script_count > 0 {
        check_ok(config, &format!("bootstrap scripts exist ({script_count})"));
    }
}

fn check_suggestions(config: Config, runtime: &ProjectRuntime) {
    for suggestion in &runtime.suggestions {
        check_line(
            config,
            "suggestion:",
            LabelKind::Warn,
            &format!(
                "review {} {}: {}",
                suggestion.kind, suggestion.path, suggestion.reason
            ),
        );
        if suggestion.kind == "bootstrap" {
            check_hint(
                config,
                &format!(
                    "add `{}` to the bootstrap block in robo.nix only if this project should run it automatically",
                    suggestion.path
                ),
            );
        } else {
            check_hint(
                config,
                &format!(
                    "add `{}` to requiredFiles or requiredDirectories in robo.nix only if bootstrap really needs it",
                    suggestion.path
                ),
            );
        }
    }
}

fn check_cuda_host(
    config: Config,
    runtime: &ProjectRuntime,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
) {
    let plan = cuda_check_plan(runtime);
    if !plan.needed() {
        return;
    }

    if plan.host_required {
        check_cuda_host_requirement(config, &plan, issues, warnings);
    }
    if plan.toolkit_required {
        check_cuda_toolkit_requirement(config, &plan, deep, issues, warnings);
    }
}

fn check_cuda_host_requirement(
    config: Config,
    plan: &CudaCheckPlan,
    issues: &mut usize,
    warnings: &mut usize,
) {
    if env::consts::OS != "linux" {
        check_error(config, issues, "CUDA environments require a Linux host");
        check_hint(
            config,
            "use a Linux NVIDIA machine for gpu-learning or isaac-learning environments",
        );
        return;
    }

    let host_cuda_version = crate::runtime::host_cuda_driver_version();
    if let Some(host_version) = host_cuda_version.as_deref() {
        check_ok(
            config,
            &format!("CUDA host driver supports {host_version}"),
        );
    } else {
        check_error(
            config,
            issues,
            "could not detect host NVIDIA driver CUDA support",
        );
        check_hint(
            config,
            "repair the host NVIDIA driver installation before using CUDA environments",
        );
    }

    if let Some(path) = crate::runtime::find_host_libcuda() {
        check_ok(config, &format!("CUDA driver library visible at {path}"));
        if env::var_os("ROBO_NIX_LIBCUDA_PATH").is_none()
            && plan.host_required
            && let Some(driver_dir) = Path::new(&path).parent()
        {
            check_hint(
                config,
                &format!(
                    "robo run/shell will add {} to the runtime automatically; set ROBO_NIX_LIBCUDA_PATH to override",
                    driver_dir.display()
                ),
            );
        }
    } else {
        check_warn(
            config,
            warnings,
            "libcuda.so.1 was not visible through ROBO_NIX_LIBCUDA_PATH, LD_LIBRARY_PATH, ldconfig, or known host driver locations",
        );
        check_hint(
            config,
            "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver",
        );
    }

    if let Some(expected_cuda_version) = plan.expected_wheel_version.as_deref() {
        if let Some(host_version) = host_cuda_version.as_deref() {
            if crate::runtime::cuda_version_less_than(&host_version, expected_cuda_version)
                == Some(true)
            {
                check_error(
                    config,
                    issues,
                    &format!(
                        "CUDA host driver mismatch: host supports {host_version}, uv.lock expects {expected_cuda_version}"
                    ),
                );
                check_hint(
                    config,
                    "upgrade the host NVIDIA driver or regenerate uv.lock with CUDA wheels supported by this host",
                );
            }
        }
    }
}

fn check_cuda_toolkit_requirement(
    config: Config,
    plan: &CudaCheckPlan,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
) {
    let Some(cuda_root) = crate::runtime::cuda_root_from_env() else {
        if deep {
            check_hint(
                config,
                "CUDA root is not visible in the current shell; deep checks will validate the runtime created by nix develop",
            );
            return;
        }
        check_warn(
            config,
            warnings,
            "CUDA root is not visible in the current shell",
        );
        check_hint(
            config,
            "robo shell sets CUDA_HOME/CUDA_PATH from the cuda-toolkit component",
        );
        if deep {
            check_hint(
                config,
                "deep checks will validate the runtime created by nix develop",
            );
        } else {
            check_hint(
                config,
                "open the runtime shell or run deep checks to validate the Nix CUDA toolkit",
            );
        }
        return;
    };
    check_ok(config, &format!("CUDA root exists at {cuda_root}"));

    let Some(expected_cuda_version) = plan.expected_wheel_version.as_deref() else {
        return;
    };

    let Some(actual_cuda_version) = crate::runtime::cuda_version_from_root() else {
        check_warn(
            config,
            warnings,
            "found CUDA root but could not detect its major.minor version",
        );
        check_hint(
            config,
            &format!(
                "run `robo shell -c \"$CUDA_HOME/bin/nvcc --version\"` to inspect this CUDA root"
            ),
        );
        return;
    };

    if actual_cuda_version == expected_cuda_version {
        check_ok(
            config,
            &format!("CUDA version alignment: {expected_cuda_version} at {cuda_root}"),
        );
    } else {
        check_error(
            config,
            issues,
            &format!(
                "CUDA mismatch: uv.lock expects {expected_cuda_version}, runtime reports {actual_cuda_version}"
            ),
        );
        check_hint(
            config,
            "point `ROBO_NIX_CUDA_ROOT` or `CUDA_HOME` to a toolkit matching expected CUDA ABI",
        );
    }
}

fn run_deep_checks(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject_dependencies: &BTreeSet<String>,
    issues: &mut usize,
    warnings: &mut usize,
) -> Result<(), ExitCode> {
    let mut progress = UiProgress::new(config, 6, "running deep checks");
    check_runtime_preview(config, warnings, Some(&mut progress));
    if let Err(code) = run_bootstrap_with_progress(config, &mut progress) {
        progress.finish();
        return Err(code);
    }
    check_runtime_tools(config, issues, &mut progress);
    check_runtime_egl_glvnd_surface(config, runtime, issues, warnings, &mut progress);
    check_runtime_cuda_build_surface(config, runtime, issues, &mut progress);
    check_runtime_probes(config, pyproject_dependencies, warnings, &mut progress);
    progress.finish();
    Ok(())
}

fn check_runtime_preview(config: Config, warnings: &mut usize, progress: Option<&mut UiProgress>) {
    let mut command = nix_command(config);
    command.arg("build");
    add_runtime_source_override(&mut command);
    command.args([".#default", "--dry-run", "--no-link", "--json"]);

    let mut progress = progress;
    let output = match progress.as_deref_mut() {
        Some(progress) => progress.output(&mut command, "checking runtime download plan"),
        None => crate::output_with_spinner(config, &mut command, "checking runtime download plan"),
    };

    let print_output =
        |warnings: &mut usize, output: Result<std::process::Output, std::io::Error>| match output {
            Ok(output) if output.status.success() => {
                let text = combined_output(&output);
                let summary = summarize_nix_dry_run_json(&output.stdout);
                if summary.is_empty() {
                    check_line(
                        config,
                        "preview:",
                        LabelKind::Status,
                        "runtime is already available or Nix reported no downloads",
                    );
                } else {
                    for line in summary {
                        check_line(config, "preview:", LabelKind::Status, &line);
                    }
                }
                if config.debug {
                    check_hint(config, &text);
                }
            }
            Ok(output) => {
                check_warn(config, warnings, "could not preview runtime downloads");
                check_hint(config, &combined_output(&output));
            }
            Err(err) => {
                check_warn(
                    config,
                    warnings,
                    &format!("failed to start Nix preview: {err}"),
                );
            }
        };

    match progress {
        Some(progress) => progress.suspend(|| print_output(warnings, output)),
        None => print_output(warnings, output),
    }
}

fn summarize_nix_dry_run_json(stdout: &[u8]) -> Vec<String> {
    let Ok(items) = serde_json::from_slice::<Vec<NixDryRunItem>>(stdout) else {
        return Vec::new();
    };
    if items.is_empty() {
        return Vec::new();
    }
    let output_count = items.iter().map(|item| item.outputs.len()).sum::<usize>();
    let mut summary = vec![format!(
        "planned builds: {} derivation{} producing {} output{}",
        items.len(),
        if items.len() == 1 { "" } else { "s" },
        output_count,
        if output_count == 1 { "" } else { "s" }
    )];
    if items.len() == 1 {
        summary.push(format!("derivation: {}", items[0].drv_path));
    }
    summary
}

fn check_runtime_tools(config: Config, issues: &mut usize, progress: &mut UiProgress) {
    let mut command = runtime_command(config, "uv", ["--version"], []);
    let output = progress.output(&mut command, "checking runtime tools");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => check_ok(config, "uv is available"),
        Ok(output) => {
            check_error(config, issues, "uv is not available in the runtime shell");
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_error(
                config,
                issues,
                &format!("failed to probe uv in runtime shell: {err}"),
            );
        }
    });
}

fn check_runtime_egl_glvnd_surface(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
    warnings: &mut usize,
    progress: &mut UiProgress,
) {
    if !runtime_has_component(runtime, "x11-gl") {
        progress.step("skipping EGL/GLVND graphics surface");
        return;
    }

    let mut command = runtime_command(config, "bash", ["-lc", egl::PROBE_SCRIPT], []);
    let output = progress.output(&mut command, "checking EGL/GLVND graphics surface");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            for finding in egl::findings(&egl::parse(&text)) {
                match finding.kind {
                    egl::FindingKind::Ok => check_ok(config, &finding.message),
                    egl::FindingKind::Warn => {
                        check_warn(config, warnings, &finding.message);
                        if let Some(hint) = finding.hint {
                            check_hint(config, hint);
                        }
                    }
                    egl::FindingKind::Error => {
                        check_error(config, issues, &finding.message);
                        if let Some(hint) = finding.hint {
                            check_hint(config, hint);
                        }
                    }
                }
            }
        }
        Ok(output) => {
            check_warn(config, warnings, "could not inspect EGL/GLVND runtime state");
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_warn(
                config,
                warnings,
                &format!("failed to probe EGL/GLVND runtime state: {err}"),
            );
        }
    });
}

fn check_runtime_cuda_build_surface(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
    progress: &mut UiProgress,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        progress.step("skipping CUDA native build surface");
        return;
    }

    let script = r#"
root="${ROBO_NIX_CUDA_ROOT:-${CUDA_HOME:-${CUDA_PATH:-}}}"
if [ -z "$root" ] || [ ! -d "$root" ]; then
  printf 'CUDA_HOME/CUDA_PATH did not point at a toolkit\n'
  exit 1
fi

missing=0
for path in \
  "$root/bin/nvcc" \
  "$root/include/cuda_runtime.h" \
  "$root/include/cuda_runtime_api.h" \
  "$root/include/nv/target" \
  "$root/lib/libcudart.so"
do
  if [ ! -e "$path" ]; then
    printf 'missing %s\n' "$path"
    missing=1
  fi
done

case ":${LIBRARY_PATH:-}:" in
  *":$root/lib:"*) ;;
  *)
    printf 'LIBRARY_PATH does not include %s/lib\n' "$root"
    missing=1
    ;;
esac

if [ "$missing" -ne 0 ]; then
  exit 1
fi

printf 'root=%s\n' "$root"
"#;

    let mut command = runtime_command(config, "bash", ["-lc", script], []);
    let output = progress.output(&mut command, "checking CUDA native build surface");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let root = text
                .lines()
                .find_map(|line| line.strip_prefix("root="))
                .unwrap_or("runtime CUDA toolkit");
            check_ok(
                config,
                &format!("CUDA native build surface is available at {root}"),
            );
        }
        Ok(output) => {
            check_error(config, issues, "CUDA native build surface is incomplete");
            check_hint(
                config,
                "Nix owns nvcc, CUDA headers, CCCL headers, and the libcudart link surface for native extension builds",
            );
            check_hint(
                config,
                "uv owns PyTorch and nvidia-* CUDA runtime wheels such as cuBLAS, cuDNN, and NCCL",
            );
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_error(
                config,
                issues,
                &format!("failed to probe CUDA native build surface: {err}"),
            );
        }
    });
}

fn check_runtime_probes(
    config: Config,
    pyproject_dependencies: &BTreeSet<String>,
    warnings: &mut usize,
    progress: &mut UiProgress,
) {
    if pyproject_dependencies.is_empty() {
        progress.step("skipping Python runtime probes");
        return;
    }
    progress.step("checking Python runtime probes");

    if has_dependency(pyproject_dependencies, &["pyqt6", "pyqt5", "pyside6"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped Qt binding import probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before GUI runtime probing",
                );
            });
        } else {
            let code = "from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)";
            let mut command = runtime_command(config, ".venv/bin/python", ["-c", code], []);
            let output = progress.output_current(&mut command, "checking PyQt6 GUI import");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => {
                    check_ok(config, "PyQt6 QtCore/QtGui/QtWidgets import works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "PyQt6 GUI import failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(
                        config,
                        "run 'uv sync' after changing Python dependencies or add missing native runtime components",
                    );
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run PyQt6 GUI probe: {err}"),
                ),
            });
        }
    }

    if has_dependency(pyproject_dependencies, &["matplotlib"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped matplotlib backend probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before matplotlib runtime probing",
                );
            });
        } else {
            let code = "import matplotlib.pyplot as plt; fig = plt.figure(); print(type(fig.canvas).__name__)";
            let mut command = runtime_command(
                config,
                ".venv/bin/python",
                ["-c", code],
                [("MPLBACKEND", "QtAgg")],
            );
            let output = progress.output_current(&mut command, "checking matplotlib QtAgg probe");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => {
                    check_ok(config, "matplotlib QtAgg smoke test works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "matplotlib QtAgg smoke test failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(
                        config,
                        "install a Qt binding such as pyqt6 and include qt6,x11-gl when using plt.show()",
                    );
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run matplotlib QtAgg probe: {err}"),
                ),
            });
        }
    }

    if has_dependency(pyproject_dependencies, &["torchcodec"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped TorchCodec import probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before video decoder runtime probing",
                );
            });
        } else {
            let code = "import torchcodec; print('torchcodec ok')";
            let mut command = runtime_command(config, ".venv/bin/python", ["-c", code], []);
            let output = progress.output_current(&mut command, "checking TorchCodec import");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => check_ok(
                    config,
                    "TorchCodec import works with the runtime FFmpeg libraries",
                ),
                Ok(output) => {
                    check_warn(config, warnings, "TorchCodec import failed");
                    check_hint(
                        config,
                        "TorchCodec needs FFmpeg shared libraries from the media component",
                    );
                    check_hint(config, &combined_output(&output));
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run TorchCodec import probe: {err}"),
                ),
            });
        }
    }
}

fn check_lock_freshness(config: Config, warnings: &mut usize) {
    let Ok(flake) = fs::read_to_string("flake.nix") else {
        return;
    };
    let Some(source) = flake.lines().find_map(|line| {
        line.trim()
            .strip_prefix("inputs.robo-nix.url")
            .and_then(quoted_value)
    }) else {
        return;
    };
    let Some(path) = source.strip_prefix("path:") else {
        return;
    };
    let source_path = Path::new(path);
    if source_path.join(".git").is_dir()
        && Command::new("git")
            .arg("-C")
            .arg(source_path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .is_ok_and(|output| !output.stdout.is_empty())
    {
        check_warn(
            config,
            warnings,
            "robo-nix path input has local changes; flake.lock may point at an older source snapshot",
        );
        check_hint(
            config,
            "run 'nix flake lock --update-input robo-nix' after local robo-nix edits",
        );
    }
}

fn runtime_output_with_spinner<const N: usize, const M: usize>(
    config: Config,
    message: &str,
    program: &str,
    args: [&str; N],
    envs: [(&str, &str); M],
) -> Result<std::process::Output, std::io::Error> {
    let mut command = runtime_command(config, program, args, envs);
    crate::output_with_spinner(config, &mut command, message)
}

fn runtime_command<const N: usize, const M: usize>(
    config: Config,
    program: &str,
    args: [&str; N],
    envs: [(&str, &str); M],
) -> Command {
    let mut command = command_for_runtime(config);
    command.arg("develop");
    add_runtime_source_override(&mut command);
    command.arg("-c").arg(program).args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    command
}

fn has_dependency(dependencies: &BTreeSet<String>, names: &[&str]) -> bool {
    crate::pyproject::has_dependency_name(dependencies, names.iter().copied())
}

fn check_field(config: Config, message: &str) {
    if let Some((name, value)) = message.split_once('=') {
        println!("{}={}", label(config, name, LabelKind::Hint), value);
    } else {
        println!("{}", label(config, message, LabelKind::Status));
    }
}

fn check_line(config: Config, tag: &str, kind: LabelKind, message: &str) {
    println!("{} {}", label(config, tag, kind), inline(config, message));
}

fn check_ok(config: Config, message: &str) {
    check_line(config, "ok:", LabelKind::Ok, message);
}

fn check_warn(config: Config, warnings: &mut usize, message: &str) {
    *warnings += 1;
    check_line(config, "warn:", LabelKind::Warn, message);
}

fn check_error(config: Config, issues: &mut usize, message: &str) {
    *issues += 1;
    check_line(config, "error:", LabelKind::Error, message);
}

fn check_hint(config: Config, message: &str) {
    for line in message.lines() {
        check_line(config, "hint:", LabelKind::Hint, line);
    }
}

fn check_why(config: Config, message: &str) {
    check_line(config, "why:", LabelKind::Why, message);
}

fn check_next(config: Config, message: &str) {
    check_line(config, "next:", LabelKind::Status, message);
}

fn check_status(
    config: Config,
    status: &str,
    status_kind: LabelKind,
    issues: usize,
    warnings: usize,
) {
    let mut output = format!(
        "{}{}",
        label(config, "status=", LabelKind::Hint),
        label(config, status, status_kind)
    );
    if issues > 0 {
        output.push(' ');
        output.push_str(&label(config, "issues=", LabelKind::Hint));
        output.push_str(&label(config, &issues.to_string(), LabelKind::Error));
    }
    output.push(' ');
    output.push_str(&label(config, "warnings=", LabelKind::Hint));
    output.push_str(&label(config, &warnings.to_string(), LabelKind::Warn));
    println!("{output}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(components: &[&str], cuda_wheel_version: Option<&str>) -> ProjectRuntime {
        ProjectRuntime {
            schema_version: Some("1".to_string()),
            env_name: "test".to_string(),
            python_version: "3.11".to_string(),
            cuda_wheel_version: cuda_wheel_version.map(ToOwned::to_owned),
            components: components.iter().map(|item| item.to_string()).collect(),
            suggestions: Vec::new(),
        }
    }

    #[test]
    fn cuda_wheels_require_host_but_not_toolkit() {
        let plan =
            cuda_check_plan_from_expected(&runtime(&[], Some("12.8")), Some("12.8".into()));

        assert!(plan.host_required);
        assert!(!plan.toolkit_required);
        assert_eq!(plan.expected_wheel_version.as_deref(), Some("12.8"));
    }

    #[test]
    fn cuda_toolkit_requires_build_surface_but_not_host_by_itself() {
        let plan = cuda_check_plan_from_expected(&runtime(&["cuda-toolkit"], None), None);

        assert!(!plan.host_required);
        assert!(plan.toolkit_required);
        assert_eq!(plan.expected_wheel_version, None);
    }

    #[test]
    fn isaac_sim_requires_cuda_host_even_before_lockfile_exists() {
        let plan = cuda_check_plan_from_expected(&runtime(&["isaac-sim"], None), None);

        assert!(plan.host_required);
        assert!(!plan.toolkit_required);
    }

    #[test]
    fn mujoco_gl_override_is_reported_for_mujoco_runtimes() {
        assert_eq!(
            forced_mujoco_gl(&runtime(&["mujoco"], None), Some(OsStr::new("egl"))),
            Some("egl".to_string())
        );
        assert_eq!(
            forced_mujoco_gl(&runtime(&["mujoco"], None), Some(OsStr::new(""))),
            None
        );
        assert_eq!(
            forced_mujoco_gl(&runtime(&[], None), Some(OsStr::new("egl"))),
            None
        );
    }

    #[test]
    fn reads_top_level_uv_lock_package_names() {
        let lock = r#"
[[package]]
name = "cmake"
version = "4.1.3"
dependencies = [
  { name = "not-a-package-entry" },
]

[[package]]
name = "Ninja"
version = "1.13.0"
"#;

        let names = uv_lock_package_names(lock);

        assert!(names.contains("cmake"));
        assert!(names.contains("ninja"));
        assert!(!names.contains("not-a-package-entry"));
    }

}
