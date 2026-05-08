use clap::{Args, ValueEnum};
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::diagnose::id;
use crate::runtime::{
    ProjectRuntime, RuntimeWhy, WhyEntry, build_runtime_why, read_project_runtime,
};
use crate::{
    Config, LabelKind, UiSpinner, add_runtime_source_override, combined_output,
    command_for_runtime, ensure_project_runtime, error, inline,
    label, quoted_value, section,
};

mod cuda;
mod deep;
mod egl;
mod native;
mod output;
mod python;
mod summary;

use cuda::{check_cuda_host, cuda_check_plan};
use deep::run_deep_checks;
use native::{check_native_tool_wheel_shims, native_tool_wheel_shims};
use output::{
    check_diagnostic_line, check_error_diag, check_field, check_hint, check_line, check_next,
    check_ok, check_status, check_warn_diag, check_why, check_why_item,
};
use python::{
    check_python_environment, check_python_files, python_environment_origin, PythonEnvironmentOrigin,
};
use summary::{forced_mujoco_gl, run_summary};

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
        return print_runtime_why_json(config, &why);
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
            return print_runtime_why_json(config, &why);
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
        print_command(config, "robo build");
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

    if let Some(mujoco_gl) = forced_mujoco_gl(
        runtime,
        env::var_os("MUJOCO_GL").as_deref(),
        env::var_os("ROBO_NIX_MUJOCO_GL_DEFAULT").as_deref(),
    ) {
        println!("Graphics runtime is blocked by MUJOCO_GL.\n");
        println!("Found MUJOCO_GL={mujoco_gl}.");
        println!("Desktop GLFW viewers usually need this unset.");
        println!();
        println!("Try:");
        print_command(config, "unset MUJOCO_GL");
        print_command(config, "robo check graphics");
        return ExitCode::from(1);
    }

    if !has_display_gl(runtime) {
        println!("Graphics runtime is not selected for this project.\n");
        println!(
            "This runtime appears to need desktop graphics, but robo.nix does not include `desktop-gl`."
        );
        println!();
        println!("Add `desktop-gl` to components in robo.nix, then run:");
        print_command(config, "robo build");
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
    let probe = egl::parse(&text);
    let findings = egl::findings(&probe);
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

    let mut mujoco_context = if runtime_has_component(runtime, "mujoco") {
        egl::MujocoContext::SkippedMissingVenv
    } else {
        egl::MujocoContext::NotSelected
    };
    if let Some(code) = check_mujoco_context(
        config,
        runtime,
        verbose,
        &mut mujoco_context,
    ) {
        return code;
    }

    if let Some(finding) = warnings.first() {
        println!("graphics  warning\n");
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

    println!("graphics  ok\n");
    print_graphics_summary(config, &egl::summary_sections(&probe, mujoco_context));
    if verbose {
        print_graphics_evidence(config, &findings);
    }
    if mujoco_context == egl::MujocoContext::Ready {
        println!();
        section(config, "when copied MuJoCo logs still fail");
        println!(
            "  {}",
            inline(
                config,
                "this check proves the current robo runtime can create a MuJoCo OpenGL context"
            )
        );
        println!(
            "  {}",
            inline(
                config,
                "the failing command is probably using a different environment, stale shell, or direct Python entrypoint"
            )
        );
        println!();
        section(config, "try");
        print_command(config, "robo run <your command>");
        print_command(config, "robo shell");
        print_command(config, "python -c 'import os; print(os.environ.get(\"MUJOCO_GL\"))'");
    }
    ExitCode::SUCCESS
}

const MUJOCO_CONTEXT_PROBE: &str = r#"
import mujoco
from mujoco import gl_context

ctx = gl_context.GLContext(64, 64)
try:
    ctx.make_current()
    model = mujoco.MjModel.from_xml_string(
        '<mujoco><worldbody><geom type="sphere" size="0.1"/></worldbody></mujoco>'
    )
    mujoco.MjrContext(model, mujoco.mjtFontScale.mjFONTSCALE_100)
    print("mujoco opengl context ok")
finally:
    ctx.free()
"#;

fn check_mujoco_context(
    config: Config,
    runtime: &ProjectRuntime,
    verbose: bool,
    context: &mut egl::MujocoContext,
) -> Option<ExitCode> {
    if !runtime_has_component(runtime, "mujoco") {
        return None;
    }
    if !Path::new(".venv/bin/python").exists() {
        if verbose {
            println!("MuJoCo OpenGL context probe skipped because .venv is missing.");
        }
        return None;
    }

    let default_output = runtime_output_with_spinner(
        config,
        "graphics: probing MuJoCo OpenGL context",
        ".venv/bin/python",
        ["-c", MUJOCO_CONTEXT_PROBE],
        [],
    );
    match default_output {
        Ok(output) if output.status.success() => {
            *context = egl::MujocoContext::Ready;
            None
        }
        Ok(default_output) => {
            let egl_output = runtime_output_with_spinner(
                config,
                "graphics: probing MuJoCo EGL context",
                "env",
                ["MUJOCO_GL=egl", ".venv/bin/python", "-c", MUJOCO_CONTEXT_PROBE],
                [],
            );
            match egl_output {
                Ok(egl_output) if egl_output.status.success() => {
                    println!("Graphics runtime needs MuJoCo GL mode selection.\n");
                    check_diagnostic_line(
                        config,
                        "warn",
                        LabelKind::Warn,
                        id::GRAPHICS_EGL_CONTEXT,
                        "MuJoCo OpenGL context failed with current GL settings",
                    );
                    println!(
                        "{} {}",
                        label(config, "ok:", LabelKind::Ok),
                        inline(config, "MuJoCo OpenGL context works with MUJOCO_GL=egl")
                    );
                    println!();
                    println!("Use EGL for offscreen MuJoCo rendering:");
                    print_command(config, "MUJOCO_GL=egl <your command>");
                    if verbose {
                        println!();
                        section(config, "failed default probe");
                        print!("{}", combined_output(&default_output));
                    }
                    Some(ExitCode::SUCCESS)
                }
                Ok(egl_output) => {
                    println!("Graphics runtime is blocked for MuJoCo.\n");
                    check_diagnostic_line(
                        config,
                        "error",
                        LabelKind::Error,
                        id::GRAPHICS_EGL_CONTEXT,
                        "MuJoCo OpenGL context probe failed",
                    );
                    println!();
                    println!("Default GL probe:");
                    print!("{}", combined_output(&default_output));
                    println!();
                    println!("EGL probe:");
                    print!("{}", combined_output(&egl_output));
                    Some(ExitCode::from(1))
                }
                Err(err) => {
                    println!("Graphics runtime is blocked for MuJoCo.\n");
                    check_diagnostic_line(
                        config,
                        "error",
                        LabelKind::Error,
                        id::GRAPHICS_EGL_CONTEXT,
                        &format!("failed to run MuJoCo EGL context probe: {err}"),
                    );
                    if verbose {
                        println!();
                        section(config, "failed default probe");
                        print!("{}", combined_output(&default_output));
                    }
                    Some(ExitCode::from(1))
                }
            }
        }
        Err(err) => {
            println!("Graphics runtime is blocked for MuJoCo.\n");
            check_diagnostic_line(
                config,
                "error",
                LabelKind::Error,
                id::GRAPHICS_EGL_CONTEXT,
                &format!("failed to run MuJoCo OpenGL context probe: {err}"),
            );
            Some(ExitCode::from(1))
        }
    }
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
        print_command(config, "robo build");
        return ExitCode::from(1);
    }

    if !tools.is_empty() {
        println!("native tools  warning\n");
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
            println!("native build  ok\n");
            println!("compiler, CMake, pkg-config, and common C/C++ build tooling are available.");
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
            print_command(config, "robo build");
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
    [
        "desktop-gl",
        "mujoco",
        "qt6",
        "matplotlib-qt",
        "isaac-sim",
    ]
        .iter()
        .any(|component| runtime_has_component(runtime, component))
}

fn has_display_gl(runtime: &ProjectRuntime) -> bool {
    runtime_has_component(runtime, "desktop-gl")
}

fn native_relevant(runtime: &ProjectRuntime) -> bool {
    ["native-build", "mujoco", "qt6", "cuda-toolkit", "isaac-sim", "ros2-jazzy"]
        .iter()
        .any(|component| runtime_has_component(runtime, component))
}

fn print_command(config: Config, command: &str) {
    println!("  {}", label(config, command, LabelKind::Command));
}

fn print_graphics_summary(config: Config, sections: &[egl::SummarySection]) {
    for (index, summary_section) in sections.iter().enumerate() {
        if index > 0 {
            println!();
        }
        section(config, summary_section.title);
        for row in &summary_section.rows {
            let status = match row.kind {
                egl::FindingKind::Ok => label(config, "ok", LabelKind::Ok),
                egl::FindingKind::Warn => label(config, "warn", LabelKind::Warn),
                egl::FindingKind::Error => label(config, "error", LabelKind::Error),
            };
            println!(
                "  {} {:<14} {}",
                status,
                label(config, row.name, LabelKind::Hint),
                inline(config, &row.value)
            );
        }
    }
}

fn print_graphics_evidence(config: Config, findings: &[egl::Finding]) {
    println!();
    section(config, "details");
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
    print_why_group(config, "requirements", &why.requirements);
    print_why_group(config, "components", &why.components);
    print_why_group(config, "required directories", &why.required_directories);
    print_why_group(config, "required files", &why.required_files);
    print_why_group(config, "bootstrap scripts", &why.bootstrap_scripts);
    print_why_group(config, "suggestions", &why.suggestions);
}

fn print_runtime_why_json(config: Config, why: &RuntimeWhy) -> ExitCode {
    match serde_json::to_string_pretty(why) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error(
                config,
                &format!("failed to encode runtime provenance: {err}"),
            );
            ExitCode::from(1)
        }
    }
}

fn runtime_has_component(runtime: &ProjectRuntime, component: &str) -> bool {
    runtime.components.iter().any(|item| item == component)
}

fn check_graphics_environment(config: Config, runtime: &ProjectRuntime, warnings: &mut usize) {
    let Some(mujoco_gl) = forced_mujoco_gl(
        runtime,
        env::var_os("MUJOCO_GL").as_deref(),
        env::var_os("ROBO_NIX_MUJOCO_GL_DEFAULT").as_deref(),
    ) else {
        return;
    };

    check_warn_diag(
        config,
        warnings,
        id::GRAPHICS_MUJOCO_GL_FORCED,
        &format!("MuJoCo GL backend forced by MUJOCO_GL={mujoco_gl}"),
    );
    check_hint(
        config,
        "desktop GLFW viewers usually need MUJOCO_GL unset; headless probes may set it narrowly",
    );
}

fn print_why_group(config: Config, title: &str, entries: &[WhyEntry]) {
    if entries.is_empty() {
        return;
    }
    check_why(config, title);
    for entry in entries {
        check_why_item(
            config,
            &format!("{} <- {}: {}", entry.name, entry.source, entry.reason),
        );
    }
}

fn check_schema_version(config: Config, runtime: &ProjectRuntime, warnings: &mut usize) {
    match runtime.schema_version.as_deref() {
        Some("1") => check_ok(config, "robo.nix schema version is 1"),
        Some(version) => {
            check_warn_diag(
                config,
                warnings,
                id::RUNTIME_FILES_MISSING_OR_STALE,
                &format!("robo.nix schema version {version} is newer than this robo supports"),
            );
            check_hint(
                config,
                "upgrade robo-nix or regenerate with `robo init . --force` after reviewing local edits",
            );
        }
        None => {
            check_warn_diag(
                config,
                warnings,
                id::RUNTIME_FILES_MISSING_OR_STALE,
                "robo.nix schema version is missing",
            );
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
        check_warn_diag(
            config,
            warnings,
            id::PYTHON_PROJECT_FILES_MISSING,
            "uv.lock is missing",
        );
        check_hint(
            config,
            "run 'robo shell', then run 'uv sync' after defining pyproject.toml dependencies",
        );
    }

    if Path::new(".venv").is_dir() {
        check_ok(config, "uv virtual environment exists");
    } else {
        check_warn_diag(
            config,
            warnings,
            id::PYTHON_ENV_MISSING,
            "uv virtual environment is missing",
        );
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
            check_warn_diag(
                config,
                warnings,
                id::RUNTIME_COMPONENTS_INCOMPLETE,
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
            check_error_diag(
                config,
                issues,
                id::PROJECT_REQUIRED_DIRECTORIES_MISSING,
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
            check_error_diag(
                config,
                issues,
                id::RUNTIME_FILES_MISSING_OR_STALE,
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
            check_error_diag(
                config,
                issues,
                id::RUNTIME_FILES_MISSING_OR_STALE,
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
                "delete this stale bootstrap suggestion; bootstrap blocks should come from explicit project policy",
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
        check_warn_diag(
            config,
            warnings,
            id::RUNTIME_FILES_MISSING_OR_STALE,
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

pub(super) fn runtime_command<const N: usize, const M: usize>(
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

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
    fn mujoco_gl_override_is_reported_for_mujoco_runtimes() {
        assert_eq!(
            forced_mujoco_gl(&runtime(&["mujoco"], None), Some(OsStr::new("glfw")), None),
            Some("glfw".to_string())
        );
        assert_eq!(
            forced_mujoco_gl(
                &runtime(&["mujoco"], None),
                Some(OsStr::new("egl")),
                Some(OsStr::new("egl"))
            ),
            None
        );
        assert_eq!(
            forced_mujoco_gl(&runtime(&["mujoco"], None), Some(OsStr::new("egl")), None),
            Some("egl".to_string())
        );
        assert_eq!(
            forced_mujoco_gl(&runtime(&["mujoco"], None), Some(OsStr::new("")), None),
            None
        );
        assert_eq!(
            forced_mujoco_gl(&runtime(&[], None), Some(OsStr::new("egl")), None),
            None
        );
    }

}
