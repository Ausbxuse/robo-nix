use clap::Args;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::runtime::{build_runtime_why, read_project_runtime, ProjectRuntime, RuntimeWhy, WhyEntry};
use crate::{
    combined_output, command_for_runtime, error, exact_python_requirement, ensure_project_runtime,
    label, nix_command, quoted_value, run_bootstrap, status, Config, LabelKind,
};

#[derive(Args)]
pub(crate) struct DoctorArgs {
    #[arg(long, help = "Run runtime probes that may realize larger Nix closures")]
    deep: bool,

    #[arg(long, help = "Explain why runtime entries are present")]
    why: bool,

    #[arg(long, requires = "why", help = "Emit machine-readable provenance")]
    json: bool,
}

pub(crate) fn run(args: DoctorArgs, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let runtime = read_project_runtime();
    let why = build_runtime_why(&runtime);
    if args.why && args.json {
        match serde_json::to_string_pretty(&why) {
            Ok(text) => {
                println!("{text}");
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                error(config, &format!("failed to encode runtime provenance: {err}"));
                return ExitCode::from(1);
            }
        }
    }

    let pyproject = fs::read_to_string("pyproject.toml").ok();
    let pyproject_lower = pyproject.as_deref().map(str::to_ascii_lowercase);
    let mut issues = 0;
    let mut warnings = 0;

    doctor_field(config, &format!("env={}", runtime.env_name));
    doctor_field(config, &format!("python={}", runtime.python_version));
    doctor_field(
        config,
        &format!(
            "workspace={}",
            env::current_dir().map_or_else(|_| ".".into(), |path| path.display().to_string())
        ),
    );

    doctor_ok(config, "workspace root exists");
    doctor_schema_version(config, &runtime, &mut warnings);
    doctor_lock_freshness(config, &mut warnings);
    doctor_python_files(config, &runtime, pyproject.as_deref(), &mut issues, &mut warnings);
    doctor_uv_files(config, &mut warnings);
    doctor_inferred_components(config, &runtime, pyproject_lower.as_deref(), &mut warnings);
    doctor_cuda_host(config, &runtime, &mut issues);
    doctor_suggestions(config, &runtime);
    if args.why {
        print_runtime_why(config, &why);
    }

    if args.deep {
        doctor_runtime_preview(config, &mut warnings);
        if let Err(code) = run_bootstrap(config) {
            return code;
        }
        doctor_runtime_tools(config, &mut issues);
        doctor_runtime_probes(config, pyproject_lower.as_deref(), &mut warnings);
    } else {
        doctor_warn(config, &mut warnings, "deep runtime checks skipped");
        doctor_hint(
            config,
            "run 'robo doctor --deep' to probe uv, PyQt, matplotlib, and native runtime libraries",
        );
    }

    if issues == 0 {
        if args.deep {
            doctor_next(config, "run 'robo dry-run' if you want a bootstrap-only validation pass");
        } else {
            doctor_next(
                config,
                "run 'robo doctor --deep' before debugging native runtime failures",
            );
        }
        doctor_next(config, "run 'robo develop' to enter the environment");
        doctor_status(config, &format!("ok warnings={warnings}"), LabelKind::Ok);
        ExitCode::SUCCESS
    } else {
        doctor_next(config, "fix the issues above and rerun 'robo doctor'");
        doctor_status(
            config,
            &format!("error issues={issues} warnings={warnings}"),
            LabelKind::Error,
        );
        ExitCode::from(1)
    }
}

fn print_runtime_why(config: Config, why: &RuntimeWhy) {
    doctor_why(
        config,
        &format!("profile={}", why.profile.as_deref().unwrap_or("manual/unknown")),
    );
    for entry in &why.components {
        print_why_entry(config, "component", entry);
    }
    for entry in &why.required_directories {
        print_why_entry(config, "required-directory", entry);
    }
    for entry in &why.required_files {
        print_why_entry(config, "required-file", entry);
    }
    for entry in &why.bootstrap_scripts {
        print_why_entry(config, "bootstrap-script", entry);
    }
    for entry in &why.suggestions {
        print_why_entry(config, "suggestion", entry);
    }
}

fn print_why_entry(config: Config, kind: &str, entry: &WhyEntry) {
    doctor_why(
        config,
        &format!(
            "{kind}={} source={} reason={}",
            entry.name, entry.source, entry.reason
        ),
    );
    doctor_hint(config, &entry.remove_hint);
    doctor_hint(config, &entry.remediation_hint);
}

fn doctor_python_files(
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
                doctor_ok(config, &format!(".python-version matches {}", runtime.python_version));
            } else {
                doctor_warn(
                    config,
                    warnings,
                    &format!(
                        ".python-version is {version} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                doctor_hint(config, "update .python-version or pythonVersion in robo.nix");
            }
        }
        Err(_) => {
            doctor_warn(config, warnings, ".python-version is missing");
            doctor_hint(config, &format!(
                "create .python-version with {} so uv uses the intended interpreter",
                runtime.python_version
            ));
        }
    }

    if let Some(pyproject) = pyproject {
        if let Some(required) = exact_python_requirement(pyproject) {
            if required == runtime.python_version {
                doctor_ok(config, &format!("pyproject.toml requires Python {required}"));
            } else {
                doctor_error(
                    config,
                    issues,
                    &format!(
                        "pyproject.toml requires Python {required} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                doctor_hint(config, &format!(
                    "set `pythonVersion = \"{required}\";` in robo.nix and write `{required}` to .python-version"
                ));
            }
        }
    } else {
        doctor_warn(config, warnings, "pyproject.toml is missing");
        doctor_hint(config, "run `robo init .` or create pyproject.toml for uv");
    }
}

fn doctor_schema_version(config: Config, runtime: &ProjectRuntime, warnings: &mut usize) {
    match runtime.schema_version.as_deref() {
        Some("1") => doctor_ok(config, "robo.nix schema version is 1"),
        Some(version) => {
            doctor_warn(
                config,
                warnings,
                &format!("robo.nix schema version {version} is newer than this robo supports"),
            );
            doctor_hint(config, "upgrade robo-nix or regenerate with `robo init . --force` after reviewing local edits");
        }
        None => {
            doctor_warn(config, warnings, "robo.nix schema version is missing");
            doctor_hint(config, "rerun `robo init . --force` when you are ready to migrate this generated file");
        }
    }
}

fn doctor_uv_files(config: Config, warnings: &mut usize) {
    if Path::new("uv.lock").exists() {
        doctor_ok(config, "uv.lock is present");
    } else {
        doctor_warn(config, warnings, "uv.lock is missing");
        doctor_hint(config, "run 'uv sync' after defining pyproject.toml dependencies");
    }

    if Path::new(".venv").is_dir() {
        doctor_ok(config, "uv virtual environment exists");
    } else {
        doctor_warn(config, warnings, "uv virtual environment is missing");
        doctor_hint(config, "run 'uv sync' to create .venv");
    }
}

fn doctor_inferred_components(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject_lower: Option<&str>,
    warnings: &mut usize,
) {
    let Some(pyproject_lower) = pyproject_lower else {
        return;
    };
    if has_dependency(pyproject_lower, &["pyqt6", "pyqt5", "pyside6"]) {
        if runtime.components.iter().any(|component| component == "qt6") {
            doctor_ok(config, "Qt binding dependency has qt6 runtime component");
        } else {
            doctor_warn(config, warnings, "Qt binding dependency detected but qt6 is not selected");
            doctor_hint(config, "rerun 'robo init . --with qt6,x11-gl' or add both components to robo.nix");
        }
        if runtime.components.iter().any(|component| component == "x11-gl") {
            doctor_ok(config, "Qt binding dependency has x11-gl runtime component");
        } else {
            doctor_warn(
                config,
                warnings,
                "Qt binding dependency detected but x11-gl is not selected",
            );
            doctor_hint(config, "add x11-gl to robo.nix for Linux desktop display/OpenGL libraries");
        }
    }
}

fn doctor_suggestions(config: Config, runtime: &ProjectRuntime) {
    for path in &runtime.suggestions {
        doctor_line(
            config,
            "suggestion:",
            LabelKind::Warn,
            &format!("check whether {path} should be required for this project"),
        );
        doctor_hint(config, &format!("add `{path}` to requiredFiles or requiredDirectories in robo.nix only if bootstrap really needs it"));
    }
}

fn doctor_cuda_host(config: Config, runtime: &ProjectRuntime, issues: &mut usize) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        return;
    }

    if env::consts::OS != "linux" {
        doctor_error(config, issues, "CUDA environments require a Linux host");
        doctor_hint(config, "use a Linux NVIDIA machine for gpu-learning or isaac-learning environments");
        return;
    }
    doctor_ok(config, "Linux host detected for CUDA environment");

    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            doctor_ok(config, "NVIDIA driver stack is reachable through nvidia-smi");
        }
        Ok(_) => {
            doctor_error(
                config,
                issues,
                "nvidia-smi is present but the NVIDIA driver stack is not healthy",
            );
            doctor_hint(config, "repair the host NVIDIA driver installation before using CUDA environments");
        }
        Err(_) => {
            doctor_error(config, issues, "nvidia-smi is not available on this host");
            doctor_hint(config, "run this environment on a machine with NVIDIA drivers installed");
        }
    }

    let cuda_root = env::var("CUDA_HOME")
        .or_else(|_| env::var("CUDA_PATH"))
        .or_else(|_| env::var("ROBO_NIX_CUDA_ROOT"))
        .unwrap_or_else(|_| "/usr/local/cuda".to_string());
    if Path::new(&cuda_root).is_dir() {
        doctor_ok(config, &format!("CUDA root exists at {cuda_root}"));
    } else {
        doctor_error(config, issues, &format!("CUDA root not found at {cuda_root}"));
        doctor_hint(
            config,
            "set CUDA_HOME, CUDA_PATH, or ROBO_NIX_CUDA_ROOT if CUDA is installed in a non-default location",
        );
    }
}

fn doctor_runtime_preview(config: Config, warnings: &mut usize) {
    status(config, "checking runtime download plan");
    let mut command = nix_command(config);
    command.args(["build", ".#default", "--dry-run", "--no-link"]);

    match command.output() {
        Ok(output) if output.status.success() => {
            let text = combined_output(&output);
            let summary = summarize_nix_dry_run(&text);
            if summary.is_empty() {
                doctor_line(
                    config,
                    "preview:",
                    LabelKind::Status,
                    "runtime is already available or Nix reported no downloads",
                );
            } else {
                for line in summary {
                    doctor_line(config, "preview:", LabelKind::Status, &line);
                }
            }
            if config.debug {
                doctor_hint(config, &text);
            }
        }
        Ok(output) => {
            doctor_warn(config, warnings, "could not preview runtime downloads");
            doctor_hint(config, &combined_output(&output));
        }
        Err(err) => {
            doctor_warn(config, warnings, &format!("failed to start Nix preview: {err}"));
        }
    }
}

fn summarize_nix_dry_run(text: &str) -> Vec<String> {
    let mut summary = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with("these ") || line.starts_with("this ") {
            if line.contains("will be built") {
                summary.push(format!("local builds: {}", clean_nix_summary(line)));
            } else if line.contains("will be fetched") || line.contains("will be downloaded") {
                summary.push(format!("downloads: {}", clean_nix_summary(line)));
            }
        }
    }
    summary
}

fn clean_nix_summary(line: &str) -> String {
    line.trim_end_matches(':')
        .replace("derivations", "items")
        .replace("derivation", "item")
        .replace("paths", "items")
        .replace("path", "item")
}

fn doctor_runtime_tools(config: Config, issues: &mut usize) {
    match runtime_output(config, "uv", ["--version"], []) {
        Ok(output) if output.status.success() => doctor_ok(config, "uv is available"),
        Ok(output) => {
            doctor_error(config, issues, "uv is not available in the runtime shell");
            doctor_hint(config, &combined_output(&output));
        }
        Err(err) => {
            doctor_error(config, issues, &format!("failed to probe uv in runtime shell: {err}"));
        }
    }
}

fn doctor_runtime_probes(config: Config, pyproject_lower: Option<&str>, warnings: &mut usize) {
    let Some(pyproject_lower) = pyproject_lower else {
        return;
    };
    if has_dependency(pyproject_lower, &["pyqt6", "pyqt5", "pyside6"]) {
        if !Path::new(".venv/bin/python").exists() {
            doctor_warn(
                config,
                warnings,
                "Python virtualenv is missing; skipped Qt binding import probe",
            );
            doctor_hint(config, "run 'robo sync' to create .venv before GUI runtime probing");
        } else {
            let code = "from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)";
            match runtime_output(config, ".venv/bin/python", ["-c", code], []) {
                Ok(output) if output.status.success() => {
                    doctor_ok(config, "PyQt6 QtCore/QtGui/QtWidgets import works")
                }
                Ok(output) => {
                    doctor_warn(config, warnings, "PyQt6 GUI import failed");
                    doctor_hint(config, &combined_output(&output));
                    doctor_hint(config, "run 'robo sync' after changing Python dependencies or add missing native runtime components");
                }
                Err(err) => {
                    doctor_warn(config, warnings, &format!("failed to run PyQt6 GUI probe: {err}"))
                }
            }
        }
    }

    if has_dependency(pyproject_lower, &["matplotlib"]) {
        if !Path::new(".venv/bin/python").exists() {
            doctor_warn(
                config,
                warnings,
                "Python virtualenv is missing; skipped matplotlib backend probe",
            );
            doctor_hint(config, "run 'robo sync' to create .venv before matplotlib runtime probing");
        } else {
            let code =
                "import matplotlib.pyplot as plt; fig = plt.figure(); print(type(fig.canvas).__name__)";
            match runtime_output(
                config,
                ".venv/bin/python",
                ["-c", code],
                [("MPLBACKEND", "QtAgg")],
            ) {
                Ok(output) if output.status.success() => {
                    doctor_ok(config, "matplotlib QtAgg smoke test works")
                }
                Ok(output) => {
                    doctor_warn(config, warnings, "matplotlib QtAgg smoke test failed");
                    doctor_hint(config, &combined_output(&output));
                    doctor_hint(config, "install a Qt binding such as pyqt6 and include qt6,x11-gl when using plt.show()");
                }
                Err(err) => doctor_warn(
                    config,
                    warnings,
                    &format!("failed to run matplotlib QtAgg probe: {err}"),
                ),
            }
        }
    }
}

fn doctor_lock_freshness(config: Config, warnings: &mut usize) {
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
        doctor_warn(
            config,
            warnings,
            "robo-nix path input has local changes; flake.lock may point at an older source snapshot",
        );
        doctor_hint(config, "run 'nix flake lock --update-input robo-nix' after local robo-nix edits");
    }
}

fn runtime_output<const N: usize, const M: usize>(
    config: Config,
    program: &str,
    args: [&str; N],
    envs: [(&str, &str); M],
) -> Result<std::process::Output, std::io::Error> {
    let mut command = command_for_runtime(config);
    command.arg("develop").arg("-c").arg(program).args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output()
}

fn has_dependency(text: &str, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| text.contains(&format!("\"{name}")) || text.contains(&format!("'{name}")))
}

fn doctor_field(config: Config, message: &str) {
    println!("{} {message}", label(config, "doctor:", LabelKind::Status));
}

fn doctor_line(config: Config, tag: &str, kind: LabelKind, message: &str) {
    println!(
        "{} {} {message}",
        label(config, "doctor:", LabelKind::Status),
        label(config, tag, kind)
    );
}

fn doctor_ok(config: Config, message: &str) {
    doctor_line(config, "ok:", LabelKind::Ok, message);
}

fn doctor_warn(config: Config, warnings: &mut usize, message: &str) {
    *warnings += 1;
    doctor_line(config, "warn:", LabelKind::Warn, message);
}

fn doctor_error(config: Config, issues: &mut usize, message: &str) {
    *issues += 1;
    doctor_line(config, "error:", LabelKind::Error, message);
}

fn doctor_hint(config: Config, message: &str) {
    for line in message.lines() {
        doctor_line(config, "hint:", LabelKind::Hint, line);
    }
}

fn doctor_why(config: Config, message: &str) {
    doctor_line(config, "why:", LabelKind::Why, message);
}

fn doctor_next(config: Config, message: &str) {
    doctor_line(config, "next:", LabelKind::Status, message);
}

fn doctor_status(config: Config, message: &str, kind: LabelKind) {
    println!(
        "{} {}{message}",
        label(config, "doctor:", LabelKind::Status),
        label(config, "status=", kind)
    );
}
