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
pub(crate) struct CheckArgs {
    #[arg(long, help = "Run runtime probes that may realize larger Nix closures")]
    deep: bool,

    #[arg(long, help = "Explain why runtime entries are present")]
    why: bool,

    #[arg(long, requires = "why", help = "Emit machine-readable provenance")]
    json: bool,
}

pub(crate) fn run(args: CheckArgs, config: Config) -> ExitCode {
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
    check_python_files(config, &runtime, pyproject.as_deref(), &mut issues, &mut warnings);
    check_uv_files(config, &mut warnings);
    check_expected_components(config, &runtime, pyproject.as_deref(), &mut warnings);
    check_required_paths(config, &why, &mut issues);
    check_cuda_host(config, &runtime, &mut issues, &mut warnings);
    check_suggestions(config, &runtime);
    if args.why {
        print_runtime_why(config, &why);
    }

    if args.deep {
        check_runtime_preview(config, &mut warnings);
        if let Err(code) = run_bootstrap(config) {
            return code;
        }
        check_runtime_tools(config, &mut issues);
        check_runtime_probes(config, pyproject_lower.as_deref(), &mut warnings);
    } else {
        check_warn(config, &mut warnings, "deep runtime checks skipped");
        check_hint(
            config,
            "run 'robo check --deep' to probe uv, PyQt, matplotlib, and native runtime libraries",
        );
    }

    if issues == 0 {
        if args.deep {
            check_next(config, "run 'robo dry-run' if you want a bootstrap-only validation pass");
        } else {
            check_next(
                config,
                "run 'robo check --deep' before debugging native runtime failures",
            );
        }
        check_next(config, "run 'robo shell' to enter the environment");
        check_status(config, &format!("ok warnings={warnings}"), LabelKind::Ok);
        ExitCode::SUCCESS
    } else {
        check_next(config, "fix the issues above and rerun 'robo check'");
        check_status(
            config,
            &format!("error issues={issues} warnings={warnings}"),
            LabelKind::Error,
        );
        ExitCode::from(1)
    }
}

fn print_runtime_why(config: Config, why: &RuntimeWhy) {
    check_why(
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
    check_why(
        config,
        &format!(
            "{kind}={} source={} reason={}",
            entry.name, entry.source, entry.reason
        ),
    );
    check_hint(config, &entry.remove_hint);
    check_hint(config, &entry.remediation_hint);
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
                check_ok(config, &format!(".python-version matches {}", runtime.python_version));
            } else {
                check_warn(
                    config,
                    warnings,
                    &format!(
                        ".python-version is {version} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(config, "update .python-version or pythonVersion in robo.nix");
            }
        }
        Err(_) => {
            check_warn(config, warnings, ".python-version is missing");
            check_hint(config, &format!(
                "create .python-version with {} so uv uses the intended interpreter",
                runtime.python_version
            ));
        }
    }

    if let Some(pyproject) = pyproject {
        if let Some(required) = exact_python_requirement(pyproject) {
            if required == runtime.python_version {
                check_ok(config, &format!("pyproject.toml requires Python {required}"));
            } else {
                check_error(
                    config,
                    issues,
                    &format!(
                        "pyproject.toml requires Python {required} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(config, &format!(
                    "set `pythonVersion = \"{required}\";` in robo.nix and write `{required}` to .python-version"
                ));
            }
        }
    } else {
        check_warn(config, warnings, "pyproject.toml is missing");
        check_hint(config, "run `robo init .` or create pyproject.toml for uv");
    }
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
            check_hint(config, "upgrade robo-nix or regenerate with `robo init . --force` after reviewing local edits");
        }
        None => {
            check_warn(config, warnings, "robo.nix schema version is missing");
            check_hint(config, "rerun `robo init . --force` when you are ready to migrate this generated file");
        }
    }
}

fn check_uv_files(config: Config, warnings: &mut usize) {
    if Path::new("uv.lock").exists() {
        check_ok(config, "uv.lock is present");
    } else {
        check_warn(config, warnings, "uv.lock is missing");
        check_hint(config, "run 'uv sync' after defining pyproject.toml dependencies");
    }

    if Path::new(".venv").is_dir() {
        check_ok(config, "uv virtual environment exists");
    } else {
        check_warn(config, warnings, "uv virtual environment is missing");
        check_hint(config, "run 'uv sync' to create .venv");
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
    for expected in crate::runtime::expected_components_from_pyproject(pyproject) {
        if runtime.components.iter().any(|component| component == &expected.name) {
            check_ok(
                config,
                &format!("pyproject expectation has component {}: {}", expected.name, expected.reason),
            );
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
}

fn check_required_paths(config: Config, why: &RuntimeWhy, issues: &mut usize) {
    for path in &why.required_directories {
        if Path::new(&path.name).is_dir() {
            check_ok(config, &format!("required directory exists: {}", path.name));
        } else {
            check_error(config, issues, &format!("required directory is missing: {}", path.name));
            check_hint(config, &path.remediation_hint);
        }
    }
    for path in &why.required_files {
        if Path::new(&path.name).is_file() {
            check_ok(config, &format!("required file exists: {}", path.name));
        } else {
            check_error(config, issues, &format!("required file is missing: {}", path.name));
            check_hint(config, &path.remediation_hint);
        }
    }
    for script in &why.bootstrap_scripts {
        if Path::new(&script.name).is_file() {
            check_ok(config, &format!("bootstrap script exists: {}", script.name));
        } else {
            check_error(config, issues, &format!("bootstrap script is missing: {}", script.name));
            check_hint(config, &script.remediation_hint);
        }
    }
}

fn check_suggestions(config: Config, runtime: &ProjectRuntime) {
    for path in &runtime.suggestions {
        check_line(
            config,
            "suggestion:",
            LabelKind::Warn,
            &format!("check whether {path} should be required for this project"),
        );
        check_hint(config, &format!("add `{path}` to requiredFiles or requiredDirectories in robo.nix only if bootstrap really needs it"));
    }
}

fn check_cuda_host(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
    warnings: &mut usize,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        return;
    }

    if env::consts::OS != "linux" {
        check_error(config, issues, "CUDA environments require a Linux host");
        check_hint(config, "use a Linux NVIDIA machine for gpu-learning or isaac-learning environments");
        return;
    }
    check_ok(config, "Linux host detected for CUDA environment");

    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            check_ok(config, "NVIDIA driver stack is reachable through nvidia-smi");
        }
        Ok(_) => {
            check_error(
                config,
                issues,
                "nvidia-smi is present but the NVIDIA driver stack is not healthy",
            );
            check_hint(config, "repair the host NVIDIA driver installation before using CUDA environments");
        }
        Err(_) => {
            check_error(config, issues, "nvidia-smi is not available on this host");
            check_hint(config, "run this environment on a machine with NVIDIA drivers installed");
        }
    }

    let expected_cuda_version = runtime
        .cuda_wheel_version
        .clone()
        .or_else(crate::runtime::infer_cuda_wheel_version_from_uv_lock);
    let Some(cuda_root) = crate::runtime::cuda_root_from_env() else {
        check_error(config, issues, "CUDA root is not available");
        check_hint(
            config,
            "set CUDA_HOME, CUDA_PATH, or ROBO_NIX_CUDA_ROOT to a valid CUDA toolkit path",
        );
        return;
    };
    check_ok(config, &format!("CUDA root exists at {cuda_root}"));

    let Some(expected_cuda_version) = expected_cuda_version.as_deref() else {
        check_warn(
            config,
            warnings,
            "could not infer an expected CUDA version from uv.lock/cudaWheelVersion",
        );
        check_hint(
            config,
            "run `robo init . --force` after regenerating uv.lock to capture cudaWheelVersion.",
        );
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
            &format!("run `robo shell -c \"$CUDA_HOME/bin/nvcc --version\"` to inspect this CUDA root"),
        );
        return;
    };

    if actual_cuda_version == expected_cuda_version {
        check_ok(
            config,
            &format!(
                "CUDA version alignment: {expected_cuda_version} at {cuda_root}"
            ),
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

fn check_runtime_preview(config: Config, warnings: &mut usize) {
    status(config, "checking runtime download plan");
    let mut command = nix_command(config);
    command.args(["build", ".#default", "--dry-run", "--no-link"]);

    match command.output() {
        Ok(output) if output.status.success() => {
            let text = combined_output(&output);
            let summary = summarize_nix_dry_run(&text);
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
            check_warn(config, warnings, &format!("failed to start Nix preview: {err}"));
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

fn check_runtime_tools(config: Config, issues: &mut usize) {
    match runtime_output(config, "uv", ["--version"], []) {
        Ok(output) if output.status.success() => check_ok(config, "uv is available"),
        Ok(output) => {
            check_error(config, issues, "uv is not available in the runtime shell");
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_error(config, issues, &format!("failed to probe uv in runtime shell: {err}"));
        }
    }
}

fn check_runtime_probes(config: Config, pyproject_lower: Option<&str>, warnings: &mut usize) {
    let Some(pyproject_lower) = pyproject_lower else {
        return;
    };
    if has_dependency(pyproject_lower, &["pyqt6", "pyqt5", "pyside6"]) {
        if !Path::new(".venv/bin/python").exists() {
            check_warn(
                config,
                warnings,
                "Python virtualenv is missing; skipped Qt binding import probe",
            );
            check_hint(config, "run 'robo sync' to create .venv before GUI runtime probing");
        } else {
            let code = "from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)";
            match runtime_output(config, ".venv/bin/python", ["-c", code], []) {
                Ok(output) if output.status.success() => {
                    check_ok(config, "PyQt6 QtCore/QtGui/QtWidgets import works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "PyQt6 GUI import failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(config, "run 'robo sync' after changing Python dependencies or add missing native runtime components");
                }
                Err(err) => {
                    check_warn(config, warnings, &format!("failed to run PyQt6 GUI probe: {err}"))
                }
            }
        }
    }

    if has_dependency(pyproject_lower, &["matplotlib"]) {
        if !Path::new(".venv/bin/python").exists() {
            check_warn(
                config,
                warnings,
                "Python virtualenv is missing; skipped matplotlib backend probe",
            );
            check_hint(config, "run 'robo sync' to create .venv before matplotlib runtime probing");
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
                    check_ok(config, "matplotlib QtAgg smoke test works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "matplotlib QtAgg smoke test failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(config, "install a Qt binding such as pyqt6 and include qt6,x11-gl when using plt.show()");
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run matplotlib QtAgg probe: {err}"),
                ),
            }
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
        check_hint(config, "run 'nix flake lock --update-input robo-nix' after local robo-nix edits");
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

fn check_field(config: Config, message: &str) {
    println!("{} {message}", label(config, "check:", LabelKind::Status));
}

fn check_line(config: Config, tag: &str, kind: LabelKind, message: &str) {
    println!(
        "{} {} {message}",
        label(config, "check:", LabelKind::Status),
        label(config, tag, kind)
    );
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

fn check_status(config: Config, message: &str, kind: LabelKind) {
    println!(
        "{} {}{message}",
        label(config, "check:", LabelKind::Status),
        label(config, "status=", kind)
    );
}
