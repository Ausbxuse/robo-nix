use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Output};
use std::time::{SystemTime, UNIX_EPOCH};

mod bootstrap;
mod error;
mod inference;
mod nix_env;
mod search;
mod shell_launch;
mod shell_refresh;
mod ui;

use bootstrap::{prepare_project, print_bootstrap_report};
use error::{print_error, write_debug_log, AppError};
use inference::dependency_evidence_from_pyproject;
use nix_env::{
    append_host_cuda_driver_bridge, apply_env, cache_runtime_environment, runtime_environment,
};
use shell_launch::interactive_shell_launch;
use shell_refresh::{runtime_input_state, runtime_input_state_for_env, set_active_shell_env};
use ui::{attention, debug, detail, help_row, list_item, section, status, Config};

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
        "__shell-refresh" => Ok(shell_refresh::run(args.collect(), config)),
        "-h" | "--help" | "help" => {
            print_usage(config);
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
    help_row(config, "robo shell", "open an interactive runtime shell");
    help_row(
        config,
        "robo run <command>",
        "run a command inside the prepared runtime",
    );
    help_row(
        config,
        "robo search <library>",
        "find a Nix runtime library package",
    );

    println!();
    section(config, "project setup");
    list_item(config, ".python-version is required.");
    list_item(config, "pyproject.toml is managed by uv/project policy.");
    list_item(
        config,
        "robo shell creates missing robo runtime files on first use.",
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
    if !args.is_empty() {
        return Err(AppError::user(
            "shell does not accept arguments; use `robo run` for commands",
        ));
    }
    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        return Err(nested_shell_error());
    }
    run_nix_develop(Vec::new(), config)
}

fn nested_shell_error() -> AppError {
    AppError::user("already inside a robo shell")
        .with_hint("exit the current shell before running `robo shell` again.")
}

fn run_command(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if args.is_empty() {
        return Err(AppError::user("run requires a command"));
    }
    run_nix_develop(args, config)
}

fn run_nix_develop(command_args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let phase = if command_args.is_empty() {
        "shell"
    } else {
        "run"
    };
    let workspace = workspace_root()?;
    let mut run_report = LastRunReport::new(phase, &workspace, &command_args);

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

    let cache_state = runtime_input_state(&workspace);
    let mut runtime_env = match runtime_environment(config, phase, &workspace, cache_state.key()) {
        Ok(runtime_env) => runtime_env,
        Err(error) => {
            run_report.errors.push(error_fact(&error));
            write_last_run_report(config, &workspace, &run_report);
            return Err(error);
        }
    };
    let post_nix_state = runtime_input_state(&workspace);
    cache_runtime_environment(&workspace, post_nix_state.key(), &runtime_env);
    let cuda_report = append_host_cuda_driver_bridge(&mut runtime_env, &workspace);
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
    if let Some(components) = runtime_env_value(&runtime_env, "ROBO_NIX_COMPONENTS") {
        run_report.components = components
            .split(':')
            .filter(|component| !component.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(warning) = host_graphics_warning(&run_report.dependencies, &runtime_env) {
        section(config, "attention");
        attention(config, warning.summary);
        detail(config, warning.detail);
        run_report.warnings.push(warning.fact());
    }
    run_report.env_names = env_names(&runtime_env);
    write_last_run_report(config, &workspace, &run_report);

    let mut command = if command_args.is_empty() {
        shell_launch_command(config, &runtime_env, &workspace)?
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

fn shell_launch_command(
    config: Config,
    runtime_env: &[(String, String)],
    workspace: &Path,
) -> Result<Command, AppError> {
    let launch = interactive_shell_launch().ok_or_else(|| {
        AppError::project("could not determine an interactive shell to launch")
            .with_hint("set ROBO_NIX_SHELL to the shell you want robo to launch.")
    })?;
    status(config, &format!("shell: launching {}", launch.name));

    let mut command = Command::new(&launch.program);
    command.args(&launch.args);
    apply_env(&mut command, runtime_env);
    for (name, value) in launch.env {
        command.env(name, value);
    }
    set_active_shell_env(
        &mut command,
        workspace,
        &runtime_input_state_for_env(workspace, runtime_env),
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
    python_version: Option<String>,
    dependencies: Vec<String>,
    components: Vec<String>,
    decisions: Vec<String>,
    env_names: Vec<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl LastRunReport {
    fn new(phase: &str, workspace: &Path, command_args: &[OsString]) -> Self {
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
            schema_version: 1,
            timestamp_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
            command,
            workspace: workspace.display().to_string(),
            python_version: None,
            dependencies: Vec::new(),
            components: Vec::new(),
            decisions: Vec::new(),
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

#[derive(Debug, Clone, Copy)]
struct HostGraphicsWarning {
    summary: &'static str,
    detail: &'static str,
}

impl HostGraphicsWarning {
    fn fact(&self) -> String {
        format!("{}; detail={}", self.summary, self.detail)
    }
}

fn host_graphics_warning(
    dependency_facts: &[String],
    envs: &[(String, String)],
) -> Option<HostGraphicsWarning> {
    if !dependency_facts.iter().any(|fact| {
        fact.split_whitespace()
            .next()
            .is_some_and(|name| name == "isaacsim")
    }) {
        return None;
    }

    if runtime_env_value(envs, "ROBO_NIX_HOST_GRAPHICS") == Some("nvidia") {
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

    Some(HostGraphicsWarning {
        summary: "Isaac Sim can see host CUDA, but no NVIDIA host graphics policy is selected",
        detail: "add `hostGraphics = \"nvidia\";` to `robo.nix` on Linux hosts that need NVIDIA Vulkan/EGL rendering.",
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
        let mut report = LastRunReport::new("run", &workspace, &[OsString::from("python")]);
        report.python_version = Some("3.11".to_string());
        report
            .dependencies
            .push("torch from project.dependencies".to_string());
        report.components.push("native-build".to_string());
        report.decisions.push("host_cuda=not-needed".to_string());
        report.env_names.push("PATH".to_string());

        let path = write_last_run_report_inner(&workspace, &report).unwrap();
        let json = fs::read_to_string(path).unwrap();

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"command\": \"run python\""));
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
    fn host_graphics_warning_points_isaac_users_at_manifest_knob() {
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

        let warning = host_graphics_warning(&dependencies, &env).unwrap();

        assert!(warning.detail.contains("hostGraphics = \"nvidia\""));
    }

    #[test]
    fn host_graphics_warning_stays_quiet_when_nvidia_policy_is_selected() {
        let dependencies = vec!["isaacsim from project.dependencies".to_string()];
        let env = vec![
            (
                "ROBO_NIX_LIBCUDA_PATH".to_string(),
                "/run/opengl-driver/lib/libcuda.so.1".to_string(),
            ),
            ("ROBO_NIX_HOST_GRAPHICS".to_string(), "nvidia".to_string()),
        ];

        assert!(host_graphics_warning(&dependencies, &env).is_none());
    }
}
