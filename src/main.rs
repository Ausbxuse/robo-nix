use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Output};
use std::time::{SystemTime, UNIX_EPOCH};

mod bootstrap;
mod env_vars;
mod error;
mod host_cuda;
mod inference;
mod nix_env;
mod profile;
mod project_lock;
mod refresh;
mod search;
mod shell_launch;
mod shell_refresh;
mod ui;
mod update;

use bootstrap::{prepare_project, print_bootstrap_report};
use error::{print_error, write_debug_log, AppError};
use host_cuda::{append_host_cuda_driver_bridge, HostCudaReport};
use inference::dependency_evidence_from_pyproject;
use nix_env::{
    apply_env, cache_runtime_environment, prefetch_runtime_input_outputs,
    retain_runtime_environment, runtime_environment,
};
use profile::{parse_runtime_options, RuntimeProfile};
use shell_launch::interactive_shell_launch;
use shell_refresh::{
    request_active_profile_switch, runtime_input_state, runtime_input_state_for_env,
    set_active_shell_env,
};
use ui::{attention, debug, detail, help_row, list_item, row, section, status, Config};

fn main() -> ExitCode {
    let config = ui_config();
    console::set_colors_enabled(config.color);
    console::set_colors_enabled_stderr(config.color);
    match run(env::args_os().skip(1).collect(), config) {
        Ok(code) => code,
        Err(error) => {
            print_error(config, &error);
            if error.should_write_debug_log() {
                match write_debug_log(&error) {
                    Ok(path) => debug(config, &format!("wrote {}", path.display())),
                    Err(err) => debug(
                        config,
                        &format!("failed to write .robo-nix/last-error.log: {err}"),
                    ),
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn ui_config() -> Config {
    let color = env::var_os("NO_COLOR").is_none()
        && (io::stdout().is_terminal() || io::stderr().is_terminal());
    let debug = env::var_os("ROBO_NIX_DEBUG").is_some();
    Config { color, debug }
}

fn version_text() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn print_version() {
    println!("{}", version_text());
}

fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_usage(config);
        return Ok(ExitCode::SUCCESS);
    };
    let command = command
        .to_str()
        .ok_or_else(|| AppError::user("command must be valid UTF-8"))?;

    match command {
        "shell" => shell_command(args.collect(), config),
        "run" => run_command(args.collect(), config),
        "search" => Ok(search::run(args.collect(), config)),
        "refresh" => refresh::run(args.collect(), config),
        "update" => update::run(args.collect(), config),
        "__shell-refresh" => Ok(shell_refresh::run(args.collect(), config)),
        "__runtime-prefetch" => {
            let workspace = env::current_dir()
                .map_err(|err| AppError::project(format!("failed to determine workspace: {err}")))?;
            prefetch_runtime_input_outputs(&workspace, &RuntimeProfile::from_active_env())?;
            Ok(ExitCode::SUCCESS)
        }
        "-h" | "--help" | "help" => {
            print_usage(config);
            Ok(ExitCode::SUCCESS)
        }
        "-V" | "--version" => {
            print_version();
            Ok(ExitCode::SUCCESS)
        }
        "init" => Err(AppError::user("`robo init` is not a robo command").with_hint(
            "run `robo shell` from a project with .python-version; first bootstrap creates missing runtime files.",
        )),
        "check" => Err(AppError::user("`robo check` is not a robo command")
            .with_hint("run `robo shell`; setup failures include actionable diagnostics.")),
        "diagnose" => Err(AppError::user("`robo diagnose` is not a robo command").with_hint(
            "run `robo shell` or `robo run`; setup failures write .robo-nix/last-error.log.",
        )),
        other => Err(AppError::user(format!("unknown command `{other}`"))),
    }
}

fn print_usage(config: Config) {
    section(config, "usage");
    help_row(
        config,
        "robo shell [--profile <name>] [--sync]",
        "open an interactive runtime shell",
    );
    help_row(
        config,
        "robo run [--profile <name>] [--sync] [--] <command>",
        "run a command inside the prepared runtime",
    );
    help_row(
        config,
        "robo search <library>",
        "find a Nix runtime library package",
    );
    help_row(
        config,
        "robo refresh [--profile <name>]",
        "clear runtime state and refresh the active shell",
    );
    help_row(
        config,
        "robo update",
        "update robo-nix and reinstall the CLI",
    );

    println!();
    section(config, "utilities");
    help_row(config, "robo --help, -h", "show help");
    help_row(config, "robo --version, -V", "show version");

    println!();
    section(config, "project setup");
    list_item(config, ".python-version is required.");
    list_item(config, "pyproject.toml is managed by uv/project policy.");
    list_item(
        config,
        "robo shell creates robo runtime files on first use.",
    );

    println!();
    section(config, "runtime lookup");
    help_row(
        config,
        "robo search libassimp.so",
        "find packages for missing shared libraries",
    );
}

fn shell_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let (options, args) = parse_runtime_options(args)?;
    if !args.is_empty() {
        return Err(AppError::user(
            "shell does not accept arguments; use `robo run` for commands",
        ));
    }
    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        if options.sync {
            return Err(AppError::user("cannot start `robo shell --sync` inside an active robo shell")
                .with_hint("run `robo run --sync -- true` to sync the active profile environment, or exit this shell before starting a new synced shell."));
        }
        if options.profile.requested().is_some() {
            return request_active_shell_profile_switch(&options.profile, config);
        }
        return Err(nested_shell_error());
    }
    run_nix_develop(Vec::new(), options.profile, options.sync, config)
}

fn request_active_shell_profile_switch(
    profile: &RuntimeProfile,
    config: Config,
) -> Result<ExitCode, AppError> {
    let workspace = active_shell_workspace_root()?;
    request_active_profile_switch(&workspace, profile).map_err(|err| {
        AppError::project(format!(
            "failed to request active shell profile switch: {err}"
        ))
        .with_hint(
            "the current shell is still usable; run `robo refresh --profile <name>` or start a new `robo shell --profile <name>`.",
        )
    })?;

    section(config, "shell");
    row(
        config,
        "✓",
        "requested",
        &format!("active shell switch to profile `{}`", profile.selector()),
    );
    row(
        config,
        "→",
        "next",
        "press Enter or run the next command after the prompt refreshes",
    );
    Ok(ExitCode::SUCCESS)
}

fn active_shell_workspace_root() -> Result<PathBuf, AppError> {
    if let Some(workspace) = env::var_os("WORKSPACE_ROOT") {
        return Ok(PathBuf::from(workspace));
    }
    workspace_root()
}

fn nested_shell_error() -> AppError {
    AppError::user("already inside a robo shell")
        .with_hint("use `robo shell --profile <name>` to switch the active shell profile, or exit the current shell before starting a new one.")
}

fn run_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let (options, args) = parse_runtime_options(args)?;
    run_nix_develop(
        normalize_run_args(args)?,
        options.profile,
        options.sync,
        config,
    )
}

fn normalize_run_args(mut args: Vec<OsString>) -> Result<Vec<OsString>, AppError> {
    if args
        .first()
        .is_some_and(|arg| arg.as_os_str() == OsStr::new("--"))
    {
        args.remove(0);
    }
    if args.is_empty() {
        return Err(AppError::user("run requires a command").with_hint(
            "use `robo run -- <command> [args...]` when the command name begins with `-`.",
        ));
    }
    Ok(args)
}

fn run_nix_develop(
    command_args: Vec<OsString>,
    profile: RuntimeProfile,
    sync: bool,
    config: Config,
) -> Result<ExitCode, AppError> {
    let phase = if command_args.is_empty() {
        "shell"
    } else {
        "run"
    };
    let workspace = workspace_root()?;
    let mut run_report = LastRunReport::new(phase, &workspace, &command_args, &profile);
    if sync {
        run_report
            .decisions
            .push("python_sync=requested".to_string());
    }

    let report = match prepare_project(&workspace) {
        Ok(report) => report,
        Err(error) => {
            run_report.errors.push(error_fact(&error));
            write_last_run_report(config, &workspace, &run_report);
            return Err(error);
        }
    };
    run_report.python_version = Some(report.python_version().to_string());
    run_report.decisions.extend(
        report
            .wrote_files()
            .into_iter()
            .map(|file| format!("generated={file}")),
    );
    if let Some(inference) = report.inference() {
        run_report.components = inference.components.iter().cloned().collect();
        run_report
            .decisions
            .extend(inference.matches.iter().map(|matched| {
                format!(
                    "inference package={} component={} capability={} sources={} provenance={}",
                    matched.package,
                    matched.component,
                    matched.capability,
                    matched.sources.join(","),
                    matched.provenance
                )
            }));
        run_report
            .warnings
            .extend(inference.diagnostics.iter().map(inference_diagnostic_fact));
    }
    run_report.dependencies = dependency_facts(&workspace);
    print_bootstrap_report(config, &report);

    let lock_update = update::reconcile_project_lock_for_running_cli(&workspace, phase, config);
    if let Some(decision) = lock_update.decision {
        run_report.decisions.push(decision);
    }
    if let Some(warning) = lock_update.warning {
        run_report.warnings.push(warning);
    }

    let cache_state = runtime_input_state(&workspace, &profile);
    let prepared_runtime =
        match runtime_environment(config, phase, &workspace, &profile, cache_state.key()) {
            Ok(runtime_env) => runtime_env,
            Err(error) => {
                run_report.errors.push(error_fact(&error));
                write_last_run_report(config, &workspace, &run_report);
                return Err(error);
            }
        };
    let fallback_error = prepared_runtime.fallback_error().map(str::to_string);
    if let Some(error) = &fallback_error {
        run_report
            .decisions
            .push("runtime_environment=last-working-fallback".to_string());
        run_report.warnings.push(format!(
            "new runtime setup failed; used last working environment: {error}"
        ));
    }
    let mut runtime_env = prepared_runtime.into_env();
    let post_nix_state = runtime_input_state(&workspace, &profile);
    let cuda_report = append_host_cuda_driver_bridge(&mut runtime_env, &workspace);
    run_report
        .host_probes
        .push(host_cuda_probe_report(&cuda_report));
    run_report.decisions.extend(cuda_report.decision_lines());
    if cuda_report.status == "needed-missing" {
        run_report
            .warnings
            .push("host CUDA driver library was needed but not found".to_string());
    }
    if let Some(error) = &cuda_report.bridge_error {
        run_report
            .warnings
            .push(format!("host CUDA bridge could not be created: {error}"));
    }
    if config.debug {
        for line in cuda_report.decision_lines() {
            debug(config, &line);
        }
    }
    let cache_result = if fallback_error.is_some() {
        retain_runtime_environment(&workspace, &profile, &runtime_env)
    } else {
        let final_cache_state = runtime_input_state_for_env(&workspace, &runtime_env, &profile);
        cache_runtime_environment(
            &workspace,
            &profile,
            post_nix_state.key(),
            final_cache_state.key(),
            &runtime_env,
        )
    };
    if let Err(error) = cache_result {
        section(config, "attention");
        attention(config, "runtime cache could not be made offline-safe");
        detail(config, &error);
        run_report.warnings.push(error);
    }
    if let Some(components) = runtime_env_value(&runtime_env, "ROBO_NIX_COMPONENTS") {
        run_report.components = components
            .split(':')
            .filter(|component| !component.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(warning) = graphics_wrapper_warning(&run_report.dependencies, &runtime_env) {
        section(config, "attention");
        attention(config, warning.summary);
        detail(config, warning.detail);
        run_report.warnings.push(warning.fact());
    }
    run_report
        .host_probes
        .push(host_graphics_probe_report(&runtime_env));
    run_report.env_names = env_names(&runtime_env);
    if sync {
        match sync_python_environment(config, &workspace, &runtime_env) {
            Ok(()) => run_report
                .decisions
                .push("python_sync=complete".to_string()),
            Err(error) => {
                run_report.errors.push(error_fact(&error));
                write_last_run_report(config, &workspace, &run_report);
                return Err(error);
            }
        }
    }
    write_last_run_report(config, &workspace, &run_report);

    let mut command = if command_args.is_empty() {
        shell_launch_command(config, &runtime_env, &workspace, &profile)?
    } else {
        run_launch_command(command_args, &runtime_env)?
    };

    let status = match command.status() {
        Ok(status) => status,
        Err(err) => {
            let error = AppError::project(format!("failed to launch {phase} command: {err}"))
                .with_hint(
                    "review the command and make sure it exists in the robo shell environment.",
                );
            run_report.errors.push(error_fact(&error));
            write_last_run_report(config, &workspace, &run_report);
            return Err(error);
        }
    };
    run_report
        .decisions
        .push(format!("command_status={status}"));
    write_last_run_report(config, &workspace, &run_report);

    Ok(exit_code_from_status(status))
}

fn sync_python_environment(
    config: Config,
    workspace: &Path,
    runtime_env: &[(String, String)],
) -> Result<(), AppError> {
    let groups = runtime_env_value(runtime_env, "ROBO_NIX_PYTHON_GROUPS").unwrap_or("");
    let extras = runtime_env_value(runtime_env, "ROBO_NIX_PYTHON_EXTRAS").unwrap_or("");
    let target = runtime_env_value(runtime_env, "UV_PROJECT_ENVIRONMENT").unwrap_or(".venv");
    let mut detail_parts = vec![format!("target={target}")];
    if runtime_env_value(runtime_env, "ROBO_NIX_PYTHON_GROUPS_SET").is_some() {
        detail_parts.push(format!(
            "groups={}",
            if groups.is_empty() { "<none>" } else { groups }
        ));
    }
    if !extras.is_empty() {
        detail_parts.push(format!("extras={extras}"));
    }
    status(
        config,
        &format!("syncing Python env {}", detail_parts.join(" ")),
    );

    let mut command = Command::new("uv");
    command.current_dir(workspace).arg("sync").arg("--locked");
    apply_env(&mut command, runtime_env);
    let status = command.status().map_err(|err| {
        AppError::project(format!("failed to launch `uv sync --locked`: {err}")).with_hint(
            "make sure the runtime profile includes the `python-uv` component before using `--sync`.",
        )
    })?;
    if !status.success() {
        return Err(AppError::project(format!("`uv sync --locked` failed with {status}")).with_hint(
            "fix the uv lockfile or dependency issue, then rerun `robo shell --sync` or `robo run --sync ...`.",
        ));
    }
    Ok(())
}

fn shell_launch_command(
    config: Config,
    runtime_env: &[(String, String)],
    workspace: &Path,
    profile: &RuntimeProfile,
) -> Result<Command, AppError> {
    let launch = interactive_shell_launch().ok_or_else(|| {
        AppError::project("could not determine an interactive shell to launch")
            .with_hint("set ROBO_NIX_SHELL to the shell you want robo to launch.")
    })?;
    status(config, &format!("launching {}", launch.name));

    let mut command = Command::new(&launch.program);
    command.args(&launch.args);
    apply_env(&mut command, runtime_env);
    for (name, value) in launch.env {
        command.env(name, value);
    }
    set_active_shell_env(
        &mut command,
        workspace,
        profile,
        &runtime_input_state_for_env(workspace, runtime_env, profile),
        runtime_env,
    );
    Ok(command)
}

fn run_launch_command(
    command_args: Vec<OsString>,
    runtime_env: &[(String, String)],
) -> Result<Command, AppError> {
    let mut command_args = command_args.into_iter();
    let program = command_args
        .next()
        .ok_or_else(|| AppError::user("run requires a command"))?;
    let mut command = Command::new(program);
    command.args(command_args);
    apply_env(&mut command, runtime_env);
    Ok(command)
}

fn write_command_output(output: &Output) -> Result<(), AppError> {
    io::stdout()
        .write_all(&output.stdout)
        .map_err(|err| AppError::project(format!("failed to write nix stdout: {err}")))?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|err| AppError::project(format!("failed to write nix stderr: {err}")))?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, AppError> {
    env::current_dir()
        .map_err(|err| AppError::project(format!("failed to determine workspace root: {err}")))
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => ExitCode::from(code as u8),
        None => ExitCode::FAILURE,
    }
}

#[derive(Debug)]
struct LastRunReport {
    schema_version: u32,
    timestamp_unix: u64,
    command: String,
    workspace: String,
    profile: Option<String>,
    python_version: Option<String>,
    dependencies: Vec<String>,
    components: Vec<String>,
    decisions: Vec<String>,
    host_probes: Vec<HostProbeReport>,
    env_names: Vec<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl LastRunReport {
    fn new(
        phase: &str,
        workspace: &Path,
        command_args: &[OsString],
        profile: &RuntimeProfile,
    ) -> Self {
        let command = if command_args.is_empty() {
            phase.to_string()
        } else {
            format!(
                "{} {}",
                phase,
                command_args
                    .first()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<missing>".to_string())
            )
        };
        Self {
            schema_version: 2,
            timestamp_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
            command,
            workspace: workspace.display().to_string(),
            profile: profile.requested().map(str::to_string),
            python_version: None,
            dependencies: Vec::new(),
            components: Vec::new(),
            decisions: Vec::new(),
            host_probes: Vec::new(),
            env_names: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn to_json(&self) -> String {
        let mut text = String::new();
        text.push_str("{\n");
        text.push_str(&format!("  \"schema_version\": {},\n", self.schema_version));
        text.push_str(&format!("  \"timestamp_unix\": {},\n", self.timestamp_unix));
        text.push_str(&format!("  \"command\": {},\n", json_string(&self.command)));
        text.push_str(&format!(
            "  \"workspace\": {},\n",
            json_string(&self.workspace)
        ));
        text.push_str(&format!(
            "  \"profile\": {},\n",
            json_optional_string(self.profile.as_deref())
        ));
        text.push_str(&format!(
            "  \"python_version\": {},\n",
            json_optional_string(self.python_version.as_deref())
        ));
        text.push_str(&format!(
            "  \"dependencies\": {},\n",
            json_string_array(&self.dependencies)
        ));
        text.push_str(&format!(
            "  \"components\": {},\n",
            json_string_array(&self.components)
        ));
        text.push_str(&format!(
            "  \"decisions\": {},\n",
            json_string_array(&self.decisions)
        ));
        text.push_str(&format!(
            "  \"host_probes\": {},\n",
            json_host_probe_array(&self.host_probes)
        ));
        text.push_str(&format!(
            "  \"env_names\": {},\n",
            json_string_array(&self.env_names)
        ));
        text.push_str(&format!(
            "  \"warnings\": {},\n",
            json_string_array(&self.warnings)
        ));
        text.push_str(&format!(
            "  \"errors\": {}\n",
            json_string_array(&self.errors)
        ));
        text.push_str("}\n");
        text
    }
}

#[derive(Debug, Clone, Default)]
struct HostProbeReport {
    name: String,
    status: String,
    source: Option<String>,
    checked: Vec<String>,
    reasons: Vec<String>,
    path: Option<String>,
    version: Option<String>,
    bridge: Option<String>,
    env_updates: Vec<String>,
    error: Option<String>,
}

fn dependency_facts(workspace: &Path) -> Vec<String> {
    dependency_evidence_from_pyproject(workspace)
        .unwrap_or_default()
        .into_iter()
        .map(|dependency| {
            format!(
                "{} from {}",
                dependency.name,
                dependency
                    .sources
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect()
}

fn env_names(envs: &[(String, String)]) -> Vec<String> {
    let mut names = envs
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn runtime_env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    envs.iter()
        .find_map(|(candidate, value)| (candidate == name).then_some(value.as_str()))
}

fn host_cuda_probe_report(report: &HostCudaReport) -> HostProbeReport {
    HostProbeReport {
        name: "host-cuda".to_string(),
        status: report.status.clone(),
        source: report.source.clone(),
        checked: report.checked.clone(),
        reasons: report.needed_by.clone(),
        path: report.libcuda.clone(),
        version: report.driver_version.clone(),
        bridge: report.bridge.clone(),
        env_updates: report.env_updates.clone(),
        error: report.bridge_error.clone(),
    }
}

fn host_graphics_probe_report(envs: &[(String, String)]) -> HostProbeReport {
    let status = runtime_env_value(envs, "ROBO_NIX_HOST_GRAPHICS")
        .unwrap_or("unknown")
        .to_string();
    let mut checked = vec!["ROBO_NIX_HOST_GRAPHICS".to_string()];
    match status.as_str() {
        "nixos" => checked.push("/run/opengl-driver/lib".to_string()),
        "nixgl" => checked.extend(["ROBO_NIX_NIXGL", "bundled nixGL"].map(str::to_string)),
        "nixgl-nvidia" => checked.extend(
            [
                "ROBO_NIX_NIXGL",
                "ROBO_NIX_NVIDIA_VERSION",
                "nvidia-smi",
                "/proc/driver/nvidia/version",
                "nixGLNvidia",
            ]
            .map(str::to_string),
        ),
        "none" => {}
        _ => checked.push("hostGraphics auto".to_string()),
    }

    let mut env_updates = present_env_names(envs, HOST_GRAPHICS_ENV_NAMES);
    if matches!(status.as_str(), "nixos" | "nixgl" | "nixgl-nvidia")
        && runtime_env_value(envs, "LD_LIBRARY_PATH").is_some()
    {
        env_updates.push("LD_LIBRARY_PATH".to_string());
    }
    env_updates.sort();
    env_updates.dedup();

    HostProbeReport {
        name: "host-graphics".to_string(),
        status,
        source: Some("robo.nix hostGraphics".to_string()),
        checked,
        reasons: Vec::new(),
        path: runtime_env_value(envs, "ROBO_NIX_NIXGL").map(str::to_string),
        version: runtime_env_value(envs, "ROBO_NIX_NVIDIA_VERSION").map(str::to_string),
        bridge: None,
        env_updates,
        error: None,
    }
}

const HOST_GRAPHICS_ENV_NAMES: &[&str] = &[
    "ROBO_NIX_HOST_GRAPHICS",
    "LIBGL_DRIVERS_PATH",
    "LIBVA_DRIVERS_PATH",
    "GBM_BACKENDS_PATH",
    "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
    "__EGL_VENDOR_LIBRARY_FILENAMES",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__NV_PRIME_RENDER_OFFLOAD",
    "__VK_LAYER_NV_optimus",
    "VK_ICD_FILENAMES",
    "VK_DRIVER_FILES",
    "VK_LAYER_PATH",
];

fn present_env_names(envs: &[(String, String)], names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| runtime_env_value(envs, name).is_some())
        .map(|name| (*name).to_string())
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct GraphicsWrapperWarning {
    summary: &'static str,
    detail: &'static str,
}

impl GraphicsWrapperWarning {
    fn fact(&self) -> String {
        format!("{}; detail={}", self.summary, self.detail)
    }
}

fn graphics_wrapper_warning(
    dependency_facts: &[String],
    envs: &[(String, String)],
) -> Option<GraphicsWrapperWarning> {
    if !dependency_facts.iter().any(|fact| {
        fact.split_whitespace()
            .next()
            .is_some_and(|name| name == "isaacsim")
    }) {
        return None;
    }

    if matches!(
        runtime_env_value(envs, "ROBO_NIX_HOST_GRAPHICS"),
        Some("nixgl-nvidia")
    ) {
        return None;
    }

    if [
        "VK_ICD_FILENAMES",
        "VK_DRIVER_FILES",
        "__EGL_VENDOR_LIBRARY_FILENAMES",
    ]
    .into_iter()
    .filter_map(|name| runtime_env_value(envs, name))
    .any(|value| value.to_ascii_lowercase().contains("nvidia"))
    {
        return None;
    }

    if runtime_env_value(envs, "ROBO_NIX_LIBCUDA_PATH").is_none() {
        return None;
    }

    Some(GraphicsWrapperWarning {
        summary: "Isaac Sim can see host CUDA, but no NVIDIA graphics wrapper is selected",
        detail: "use `hostGraphics = \"nixgl-nvidia\";` on non-NixOS Linux hosts that need NVIDIA Vulkan/EGL rendering.",
    })
}

fn error_fact(error: &AppError) -> String {
    match error.hint() {
        Some(hint) => format!("{}; hint={hint}", error.message()),
        None => error.message().to_string(),
    }
}

fn inference_diagnostic_fact(diagnostic: &inference::InferenceDiagnostic) -> String {
    match &diagnostic.detail {
        Some(detail) => format!("{}; detail={detail}", diagnostic.summary),
        None => diagnostic.summary.clone(),
    }
}

fn write_last_run_report(config: Config, workspace: &Path, report: &LastRunReport) {
    let result = write_last_run_report_inner(workspace, report);
    if config.debug {
        match result {
            Ok(path) => debug(config, &format!("wrote {}", path.display())),
            Err(err) => debug(
                config,
                &format!("failed to write .robo-nix/last-run.json: {}", err.message()),
            ),
        }
    }
}

fn write_last_run_report_inner(
    workspace: &Path,
    report: &LastRunReport,
) -> Result<PathBuf, AppError> {
    let dir = workspace.join(".robo-nix");
    fs::create_dir_all(&dir)
        .map_err(|err| AppError::project(format!("failed to create .robo-nix/: {err}")))?;
    let path = dir.join("last-run.json");
    let tmp_path = dir.join(format!("last-run.json.tmp-{}", std::process::id()));
    fs::write(&tmp_path, report.to_json())
        .map_err(|err| AppError::project(format!("failed to write last-run.json: {err}")))?;
    fs::rename(&tmp_path, &path)
        .map_err(|err| AppError::project(format!("failed to publish last-run.json: {err}")))?;
    Ok(path)
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn json_string_array(values: &[String]) -> String {
    let body = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_host_probe_array(values: &[HostProbeReport]) -> String {
    let body = values
        .iter()
        .map(json_host_probe)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_host_probe(report: &HostProbeReport) -> String {
    format!(
        "{{\"name\": {}, \"status\": {}, \"source\": {}, \"checked\": {}, \"reasons\": {}, \"path\": {}, \"version\": {}, \"bridge\": {}, \"env_updates\": {}, \"error\": {}}}",
        json_string(&report.name),
        json_string(&report.status),
        json_optional_string(report.source.as_deref()),
        json_string_array(&report.checked),
        json_string_array(&report.reasons),
        json_optional_string(report.path.as_deref()),
        json_optional_string(report.version.as_deref()),
        json_optional_string(report.bridge.as_deref()),
        json_string_array(&report.env_updates),
        json_optional_string(report.error.as_deref()),
    )
}

fn json_string(value: &str) -> String {
    let mut text = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            character if character.is_control() => {
                text.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => text.push(character),
        }
    }
    text.push('"');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_shell_error_names_the_boundary() {
        let error = nested_shell_error();

        assert!(error.message().contains("already inside a robo shell"));
    }

    #[test]
    fn run_args_accept_one_leading_separator() {
        assert_eq!(
            normalize_run_args(os_args(&["--", "pytest", "-q"])).unwrap(),
            os_args(&["pytest", "-q"])
        );
    }

    #[test]
    fn run_args_preserve_child_separator_after_command() {
        assert_eq!(
            normalize_run_args(os_args(&["pytest", "--", "-k", "smoke"])).unwrap(),
            os_args(&["pytest", "--", "-k", "smoke"])
        );
    }

    #[test]
    fn run_args_require_command_after_separator() {
        let error = normalize_run_args(os_args(&["--"])).unwrap_err();

        assert_eq!(error.message(), "run requires a command");
    }

    #[test]
    fn version_text_uses_package_metadata() {
        assert_eq!(
            version_text(),
            format!("robo {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn command_status_is_propagated() {
        use std::os::unix::process::ExitStatusExt;

        assert_eq!(
            exit_code_from_status(ExitStatus::from_raw(7 << 8)),
            ExitCode::from(7)
        );
        assert_eq!(
            exit_code_from_status(ExitStatus::from_raw(9)),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn last_run_report_is_versioned_redacted_json() {
        let workspace = env::temp_dir().join(format!("robo-last-run-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).unwrap();
        let mut report = LastRunReport::new(
            "run",
            &workspace,
            &[OsString::from("python")],
            &RuntimeProfile::named("driver".to_string()).unwrap(),
        );
        report.python_version = Some("3.11".to_string());
        report
            .dependencies
            .push("torch from project.dependencies".to_string());
        report.components.push("native-build".to_string());
        report.decisions.push("host_cuda=not-needed".to_string());
        report.host_probes.push(HostProbeReport {
            name: "host-cuda".to_string(),
            status: "not-needed".to_string(),
            ..HostProbeReport::default()
        });
        report.env_names.push("PATH".to_string());

        let path = write_last_run_report_inner(&workspace, &report).unwrap();
        let json = fs::read_to_string(path).unwrap();

        assert!(json.contains("\"schema_version\": 2"));
        assert!(json.contains("\"command\": \"run python\""));
        assert!(json.contains("\"profile\": \"driver\""));
        assert!(json.contains("\"host_probes\": [{\"name\": \"host-cuda\""));
        assert!(json.contains("\"env_names\": [\"PATH\"]"));
        assert!(!json.contains("LD_LIBRARY_PATH="));

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn last_run_command_records_only_the_program() {
        let report = LastRunReport::new(
            "run",
            Path::new("/workspace"),
            &[
                OsString::from("python"),
                OsString::from("-c"),
                OsString::from("secret"),
            ],
            &RuntimeProfile::default(),
        );

        assert_eq!(report.command, "run python");
    }

    #[test]
    fn json_strings_escape_control_characters() {
        assert_eq!(
            json_string("quote\" slash\\ line\n"),
            "\"quote\\\" slash\\\\ line\\n\""
        );
    }

    #[test]
    fn host_probe_reports_are_typed_json() {
        let cuda = HostCudaReport {
            status: "auto-found".to_string(),
            needed_by: vec!["uv.lock:nvidia-cuda-runtime-cu12".to_string()],
            checked: vec!["ldconfig -p".to_string()],
            source: Some("ldconfig -p".to_string()),
            libcuda: Some("/run/opengl-driver/lib/libcuda.so.1".to_string()),
            driver_version: Some("580.65.06".to_string()),
            bridge: Some(".robo-nix/host-libs".to_string()),
            bridge_error: None,
            env_updates: vec!["ROBO_NIX_LIBCUDA_PATH".to_string()],
        };
        let graphics_env = vec![
            (
                "ROBO_NIX_HOST_GRAPHICS".to_string(),
                "nixgl-nvidia".to_string(),
            ),
            (
                "__EGL_VENDOR_LIBRARY_FILENAMES".to_string(),
                "/nix/store/vendor.json".to_string(),
            ),
            (
                "VK_ICD_FILENAMES".to_string(),
                "/nix/store/icd.json".to_string(),
            ),
            ("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string()),
        ];
        let probes = vec![
            host_cuda_probe_report(&cuda),
            host_graphics_probe_report(&graphics_env),
        ];
        let json = json_host_probe_array(&probes);

        assert!(json.contains("\"name\": \"host-cuda\""));
        assert!(json.contains("\"reasons\": [\"uv.lock:nvidia-cuda-runtime-cu12\"]"));
        assert!(json.contains("\"path\": \"/run/opengl-driver/lib/libcuda.so.1\""));
        assert!(json.contains("\"name\": \"host-graphics\""));
        assert!(json.contains("\"status\": \"nixgl-nvidia\""));
        assert!(json.contains("\"env_updates\": [\"LD_LIBRARY_PATH\""));
        assert!(json.contains("\"VK_ICD_FILENAMES\""));
    }

    #[test]
    fn graphics_wrapper_warning_points_isaac_users_at_manifest_knob() {
        let dependencies = vec!["isaacsim from project.dependencies".to_string()];
        let env = vec![
            (
                "ROBO_NIX_LIBCUDA_PATH".to_string(),
                "/run/opengl-driver/lib/libcuda.so.1".to_string(),
            ),
            (
                "__EGL_VENDOR_LIBRARY_FILENAMES".to_string(),
                "/nix/store/mesa/share/glvnd/egl_vendor.d/50_mesa.json".to_string(),
            ),
            ("ROBO_NIX_HOST_GRAPHICS".to_string(), "none".to_string()),
        ];

        let warning = graphics_wrapper_warning(&dependencies, &env).unwrap();

        assert!(warning.detail.contains("hostGraphics = \"nixgl-nvidia\""));
    }

    #[test]
    fn graphics_wrapper_warning_stays_quiet_when_nvidia_policy_is_selected() {
        let dependencies = vec!["isaacsim from project.dependencies".to_string()];
        let env = vec![
            (
                "ROBO_NIX_LIBCUDA_PATH".to_string(),
                "/run/opengl-driver/lib/libcuda.so.1".to_string(),
            ),
            (
                "ROBO_NIX_HOST_GRAPHICS".to_string(),
                "nixgl-nvidia".to_string(),
            ),
        ];

        assert!(graphics_wrapper_warning(&dependencies, &env).is_none());
    }

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }
}
