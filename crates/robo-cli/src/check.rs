use clap::Args;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::runtime::{build_runtime_why, read_project_runtime, ProjectRuntime, RuntimeWhy, WhyEntry};
use crate::{
    combined_output, command_for_runtime, error, exact_python_requirement, ensure_project_runtime,
    field, inline, label, nix_command, quoted_value, run_bootstrap_with_progress, section, Config,
    LabelKind, UiProgress, UiSpinner,
};

const HOST_CUDA_DRIVER_LIBS: &[&str] = &[
    "/run/opengl-driver/lib/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/nvidia/current/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/nvidia/libcuda.so.1",
    "/usr/lib/wsl/lib/libcuda.so.1",
];

#[derive(Args)]
pub(crate) struct CheckArgs {
    #[arg(long, help = "Run runtime probes that may realize larger Nix closures")]
    deep: bool,

    #[arg(long, help = "Explain why runtime entries are present")]
    why: bool,

    #[arg(long, help = "Print detailed check evidence")]
    verbose: bool,

    #[arg(long, requires = "why", help = "Emit machine-readable provenance")]
    json: bool,
}

pub(crate) fn run(args: CheckArgs, config: Config) -> ExitCode {
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
                error(config, &format!("failed to encode runtime provenance: {err}"));
                return ExitCode::from(1);
            }
        }
    }

    if !args.verbose && !args.why {
        return run_summary(args, config, runtime, why);
    }

    run_detailed(args, config, runtime, why)
}

fn run_detailed(
    args: CheckArgs,
    config: Config,
    runtime: ProjectRuntime,
    why: RuntimeWhy,
) -> ExitCode {
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
    check_cuda_host(config, &runtime, args.deep, &mut issues, &mut warnings);
    check_suggestions(config, &runtime);
    if args.why {
        print_runtime_why(config, &why);
    }

    if args.deep {
        let mut progress = UiProgress::new(config, 5, "running deep checks");
        check_runtime_preview(config, &mut warnings, Some(&mut progress));
        if let Err(code) = run_bootstrap_with_progress(config, &mut progress) {
            progress.finish();
            return code;
        }
        progress.step("checking runtime tools");
        progress.suspend(|| check_runtime_tools(config, &mut issues));
        progress.step("checking CUDA native build surface");
        progress.suspend(|| check_runtime_cuda_build_surface(config, &runtime, &mut issues));
        progress.step("checking Python runtime probes");
        progress.suspend(|| {
            check_runtime_probes(config, pyproject_lower.as_deref(), &mut warnings)
        });
        progress.finish();
    } else {
        check_hint(config, "deep runtime probes skipped");
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
        check_next(config, "run 'robo activate' to enter the environment");
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
        &format!("profile {}", why.profile.as_deref().unwrap_or("manual/unknown")),
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

fn run_summary(
    args: CheckArgs,
    config: Config,
    runtime: ProjectRuntime,
    why: RuntimeWhy,
) -> ExitCode {
    let mut progress = UiSpinner::new(config, "checking runtime status");
    let pyproject = fs::read_to_string("pyproject.toml").ok();
    let pyproject_lower = pyproject.as_deref().map(str::to_ascii_lowercase);
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
                .detail("run: robo activate")
                .detail("     uv sync"),
        );
    }

    if Path::new(".venv").is_dir() {
        ready.push("Python environment".to_string());
    } else {
        warnings += 1;
        attention.push(
            Attention::new("Python environment missing")
                .detail("run: robo activate")
                .detail("     uv sync"),
        );
    }

    if let Some(pyproject) = pyproject.as_deref() {
        let mut missing = Vec::new();
        for expected in crate::runtime::expected_components_from_pyproject(pyproject) {
            if !runtime.components.iter().any(|component| component == &expected.name) {
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
            ready.push(format!("required directories ({})", why.required_directories.len()));
        }
    } else {
        issues += missing_directories.len();
        attention.push(
            Attention::new("required directories missing")
                .detail(format!("missing: {}", missing_directories.join(", "))),
        );
    }

    summarize_cuda_host(
        &runtime,
        args.deep,
        &mut issues,
        &mut warnings,
        &mut ready,
        &mut attention,
    );
    progress.finish();

    if args.deep {
        let mut progress = UiProgress::new(config, 5, "running deep checks");
        check_runtime_preview(config, &mut warnings, Some(&mut progress));
        if let Err(code) = run_bootstrap_with_progress(config, &mut progress) {
            progress.finish();
            return code;
        }
        progress.step("checking runtime tools");
        progress.suspend(|| check_runtime_tools(config, &mut issues));
        progress.step("checking CUDA native build surface");
        progress.suspend(|| check_runtime_cuda_build_surface(config, &runtime, &mut issues));
        progress.step("checking Python runtime probes");
        progress.suspend(|| {
            check_runtime_probes(config, pyproject_lower.as_deref(), &mut warnings)
        });
        progress.finish();
    }

    print_summary(
        config,
        &runtime,
        &workspace,
        &ready,
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

fn summarize_cuda_host(
    runtime: &ProjectRuntime,
    deep: bool,
    issues: &mut usize,
    warnings: &mut usize,
    ready: &mut Vec<String>,
    attention: &mut Vec<Attention>,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        return;
    }

    if env::consts::OS != "linux" {
        *issues += 1;
        attention.push(
            Attention::new("CUDA requires a Linux host")
                .detail("fix: use a Linux NVIDIA machine for this runtime"),
        );
        return;
    }

    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => ready.push("CUDA host".to_string()),
        Ok(_) => {
            *issues += 1;
            attention.push(
                Attention::new("NVIDIA driver stack is unhealthy")
                    .detail("fix: repair host NVIDIA drivers before using CUDA"),
            );
            return;
        }
        Err(_) => {
            *issues += 1;
            attention.push(
                Attention::new("NVIDIA driver stack not found")
                    .detail("fix: run on a machine with NVIDIA drivers installed"),
            );
            return;
        }
    }

    if let Some(path) = host_cuda_driver_lib() {
        ready.push(format!("CUDA driver library ({path})"));
    } else {
        *warnings += 1;
        attention.push(
            Attention::new("CUDA driver library not found in common host locations")
                .detail("note: Nix provides the CUDA build toolkit, but libcuda.so.1 comes from the host driver"),
        );
    }

    if crate::runtime::cuda_root_from_env().is_none() {
        *warnings += 1;
        let mut item = Attention::new("CUDA toolkit not visible in this shell")
            .detail("run: robo activate")
            .detail("note: CUDA_HOME/CUDA_PATH are set inside the activated runtime");
        if !deep {
            item = item.detail("deep: robo check --deep validates the Nix CUDA toolkit");
        }
        attention.push(item);
    } else {
        ready.push("CUDA toolkit".to_string());
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
        "{} checked {}\n",
        label(config, "robo:", LabelKind::Status),
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
            format!(", {}", count_label(config, issues, "issue", LabelKind::Error))
        }
    );
}

fn count_label(config: Config, count: usize, noun: &str, kind: LabelKind) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!(
        "{} {noun}{suffix}",
        label(config, &count.to_string(), kind)
    )
}

fn summary_detail(config: Config, detail: &str) -> String {
    if let Some(command) = detail.strip_prefix("run: ") {
        format!(
            "{} {}",
            label(config, "run:", LabelKind::Hint),
            label(config, command, LabelKind::Command)
        )
    } else if let Some(command) = detail.strip_prefix("     ") {
        format!(
            "     {}",
            label(config, command, LabelKind::Command)
        )
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
        check_hint(config, "run 'robo activate', then run 'uv sync' after defining pyproject.toml dependencies");
    }

    if Path::new(".venv").is_dir() {
        check_ok(config, "uv virtual environment exists");
    } else {
        check_warn(config, warnings, "uv virtual environment is missing");
        check_hint(config, "run 'robo activate', then run 'uv sync' to create .venv");
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
        if runtime.components.iter().any(|component| component == &expected.name) {
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
            check_error(config, issues, &format!("required directory is missing: {}", path.name));
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
            check_error(config, issues, &format!("required file is missing: {}", path.name));
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
            check_error(config, issues, &format!("bootstrap script is missing: {}", script.name));
            check_hint(config, &script.remediation_hint);
        }
    }
    if script_count > 0 {
        check_ok(config, &format!("bootstrap scripts exist ({script_count})"));
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
    deep: bool,
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
    match Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            check_ok(config, "CUDA host prerequisites ok (Linux, NVIDIA driver)");
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

    if let Some(path) = host_cuda_driver_lib() {
        check_ok(config, &format!("CUDA driver library visible at {path}"));
    } else {
        check_warn(
            config,
            warnings,
            "libcuda.so.1 was not found in common host driver locations",
        );
        check_hint(
            config,
            "Nix provides the CUDA build toolkit; libcuda.so.1 must come from the NVIDIA host driver",
        );
    }

    let expected_cuda_version = runtime
        .cuda_wheel_version
        .clone()
        .or_else(crate::runtime::infer_cuda_wheel_version_from_uv_lock);
    let Some(cuda_root) = crate::runtime::cuda_root_from_env() else {
        check_warn(config, warnings, "CUDA root is not visible in the current shell");
        check_hint(
            config,
            "robo activate sets CUDA_HOME/CUDA_PATH from the cuda-toolkit component",
        );
        if deep {
            check_hint(config, "deep checks will validate the runtime created by nix develop");
        } else {
            check_hint(config, "activate the runtime or run deep checks to validate the Nix CUDA toolkit");
        }
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
            &format!("run `robo activate -c \"$CUDA_HOME/bin/nvcc --version\"` to inspect this CUDA root"),
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

fn check_runtime_preview(
    config: Config,
    warnings: &mut usize,
    progress: Option<&mut UiProgress>,
) {
    let mut command = nix_command(config);
    command.args(["build", ".#default", "--dry-run", "--no-link"]);

    let mut progress = progress;
    let output = match progress.as_deref_mut() {
        Some(progress) => progress.output(&mut command, "checking runtime download plan"),
        None => crate::output_with_spinner(config, &mut command, "checking runtime download plan"),
    };

    let print_output = |warnings: &mut usize, output: Result<std::process::Output, std::io::Error>| match output {
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
    };

    match progress {
        Some(progress) => progress.suspend(|| print_output(warnings, output)),
        None => print_output(warnings, output),
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

fn check_runtime_cuda_build_surface(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
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

    match runtime_output(config, "bash", ["-lc", script], []) {
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
            check_hint(config, "run 'robo activate', then run 'uv sync' before GUI runtime probing");
        } else {
            let code = "from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)";
            match runtime_output(config, ".venv/bin/python", ["-c", code], []) {
                Ok(output) if output.status.success() => {
                    check_ok(config, "PyQt6 QtCore/QtGui/QtWidgets import works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "PyQt6 GUI import failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(config, "run 'uv sync' after changing Python dependencies or add missing native runtime components");
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
            check_hint(config, "run 'robo activate', then run 'uv sync' before matplotlib runtime probing");
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

fn host_cuda_driver_lib() -> Option<&'static str> {
    HOST_CUDA_DRIVER_LIBS
        .iter()
        .copied()
        .find(|path| Path::new(path).is_file())
}

fn check_field(config: Config, message: &str) {
    if let Some((name, value)) = message.split_once('=') {
        println!(
            "{}={}",
            label(config, name, LabelKind::Hint),
            value
        );
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

fn check_status(config: Config, message: &str, kind: LabelKind) {
    let mut parts = message.split_whitespace();
    let status = parts.next().unwrap_or(message);
    let mut output = format!(
        "{}{}",
        label(config, "status=", LabelKind::Hint),
        label(config, status, kind)
    );
    for part in parts {
        output.push(' ');
        if let Some((name, value)) = part.split_once('=') {
            let value_kind = match name {
                "issues" => LabelKind::Error,
                "warnings" => LabelKind::Warn,
                _ => LabelKind::Status,
            };
            output.push_str(&label(config, &format!("{name}="), LabelKind::Hint));
            output.push_str(&label(config, value, value_kind));
        } else {
            output.push_str(part);
        }
    }
    println!("{output}");
}
