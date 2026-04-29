use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::{command_row, error, field, hint, label, section, status, Config, LabelKind};

use super::bootstrap::run_bootstrap;
use super::cuda_compat::ensure_runtime_cuda_compat;
use super::nix::{check_command, command_for_runtime, nix_command, run_status};
use super::python::ensure_python_version_files;

pub(crate) fn ensure_project_runtime(config: Config) -> Result<(), ExitCode> {
    if !Path::new("flake.nix").exists() || !Path::new("robo.nix").exists() {
        error(config, "this directory is not initialized for robo-nix.");
        hint(config, "run `robo init .` from the project checkout first.");
        return Err(ExitCode::from(1));
    }
    repair_managed_flake_source(config)?;

    if let Err(message) = check_command("nix") {
        error(config, &message);
        hint(config, "install Nix, then rerun this command.");
        return Err(ExitCode::from(1));
    }

    Ok(())
}

pub(crate) fn run_project_app(mode: Option<&str>, args: Vec<OsString>, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let mut command = nix_command(config);
    command.arg("run").arg(".#default");
    if let Some(mode) = mode {
        command.arg("--").arg(mode);
    }
    command.args(args);
    run_status(&mut command, config)
}

pub(crate) fn run_project_activate(args: Vec<OsString>, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let launch = normalize_shell_args(args);

    let mut command = command_for_runtime(config);
    command.env("ROBO_NIX_ACTIVATE", "1");
    command.env_remove("ROBO_NIX_QUIET");
    for (name, value) in launch.env {
        command.env(name, value);
    }
    run_status(command.arg("develop").args(launch.args), config)
}

pub(crate) fn run_project_status(config: Config) -> ExitCode {
    let state = RuntimeState::read();
    print_runtime_state(config, "status", &state);
    ExitCode::SUCCESS
}

pub(crate) fn run_project_deactivate(config: Config) -> ExitCode {
    let state = RuntimeState::read();
    if state.active {
        print_runtime_state(config, "deactivate", &state);
        println!();
        section(config, "next steps");
        command_row(config, "exit");
        println!(
            "  {}",
            label(
                config,
                "A child process cannot exit its parent shell directly.",
                LabelKind::Hint
            )
        );
    } else {
        println!("{} not activated\n", label(config, "robo:", LabelKind::Status));
        section(config, "status");
        println!("  {}", label(config, "ok", LabelKind::Ok));
    }
    ExitCode::SUCCESS
}

pub(crate) fn run_project_command(args: Vec<OsString>, config: Config) -> ExitCode {
    if args.is_empty() {
        error(config, "run needs a command.");
        hint(config, "example: robo run pytest tests");
        return ExitCode::from(2);
    }

    if let Err(code) = prepare_uv_runtime(config, "run", true) {
        return code;
    }

    let args = if args.first().is_some_and(|arg| arg == "uv")
        && args.get(1).is_some_and(|arg| arg == "run")
    {
        args.into_iter().skip(2).collect()
    } else {
        args
    };

    run_status(
        command_for_runtime(config)
            .arg("develop")
            .arg("-c")
            .arg("uv")
            .arg("run")
            .args(args),
        config,
    )
}

fn prepare_uv_runtime(config: Config, command_name: &str, cuda_strict: bool) -> Result<(), ExitCode> {
    ensure_pyproject(config, command_name)?;
    ensure_python_version_files(config)?;
    prepare_runtime(config, cuda_strict)
}

fn prepare_runtime(config: Config, cuda_strict: bool) -> Result<(), ExitCode> {
    ensure_project_runtime(config)?;
    run_bootstrap(config)?;
    ensure_runtime_cuda_compat(config, cuda_strict)
}

struct RuntimeState {
    active: bool,
    env_name: String,
    python_version: String,
    workspace: String,
    shell: Option<String>,
    prompt_prefix: Option<String>,
}

impl RuntimeState {
    fn read() -> Self {
        let active = env::var_os("ROBO_NIX_ACTIVE").is_some();
        let runtime = if Path::new("robo.nix").exists() {
            Some(crate::runtime::read_project_runtime())
        } else {
            None
        };
        let env_name = env::var("ROBO_NIX_ENV_NAME")
            .ok()
            .or_else(|| runtime.as_ref().map(|runtime| runtime.env_name.clone()))
            .unwrap_or_else(|| "unknown".to_string());
        let python_version = env::var("ROBO_NIX_PYTHON_VERSION")
            .ok()
            .or_else(|| runtime.as_ref().map(|runtime| runtime.python_version.clone()))
            .unwrap_or_else(|| "unknown".to_string());
        let workspace = env::var("WORKSPACE_ROOT").unwrap_or_else(|_| {
            env::current_dir().map_or_else(|_| ".".into(), |path| path.display().to_string())
        });

        Self {
            active,
            env_name,
            python_version,
            workspace,
            shell: env::var("SHELL").ok(),
            prompt_prefix: env::var("ROBO_NIX_PROMPT_PREFIX").ok(),
        }
    }
}

fn print_runtime_state(config: Config, action: &str, state: &RuntimeState) {
    println!(
        "{} {}\n",
        label(config, "robo:", LabelKind::Status),
        action
    );
    section(config, "runtime");
    field(config, "active", if state.active { "yes" } else { "no" });
    field(config, "env", &state.env_name);
    field(config, "python", &state.python_version);
    field(config, "workspace", &state.workspace);
    if let Some(shell) = &state.shell {
        field(config, "shell", shell);
    }
    if let Some(prefix) = &state.prompt_prefix {
        field(config, "prompt-prefix", prefix);
    }

    println!();
    section(config, "status");
    if state.active {
        println!("  {}", label(config, "activated", LabelKind::Ok));
    } else {
        println!("  {}", label(config, "not activated", LabelKind::Warn));
        println!();
        section(config, "next steps");
        command_row(config, "robo activate");
    }
}

fn ensure_pyproject(config: Config, command_name: &str) -> Result<(), ExitCode> {
    if Path::new("pyproject.toml").exists() {
        return Ok(());
    }
    error(config, &format!("{command_name} needs pyproject.toml."));
    hint(config, "run `robo init .` or create pyproject.toml for uv.");
    Err(ExitCode::from(1))
}

fn repair_managed_flake_source(config: Config) -> Result<(), ExitCode> {
    let flake_path = Path::new("flake.nix");
    let flake = match fs::read_to_string(flake_path) {
        Ok(flake) => flake,
        Err(err) => {
            error(config, &format!("failed to read flake.nix: {err}"));
            return Err(ExitCode::from(1));
        }
    };
    if !flake.contains("mkProjectFlakeFromManifest") {
        error(config, "flake.nix does not look generated by robo.");
        hint(
            config,
            "run `robo init . --force` only if you want robo to replace this flake.",
        );
        return Err(ExitCode::from(1));
    }
    if !flake.contains("github:ausbxuse/robo-nix") {
        return Ok(());
    }

    let source_url = match env::var("ROBO_NIX_DEFAULT_SOURCE_URL") {
        Ok(source_url) => source_url,
        Err(_) => {
            error(
                config,
                "flake.nix points at github:ausbxuse/robo-nix, but this robo install has no packaged source URL.",
            );
            hint(
                config,
                "run `robo init . --robo-nix-url path:/path/to/robo-nix` to repair this project.",
            );
            return Err(ExitCode::from(1));
        }
    };
    let repaired = flake.replace("github:ausbxuse/robo-nix", &source_url);
    if let Err(err) = fs::write(flake_path, repaired) {
        error(config, &format!("failed to repair flake.nix: {err}"));
        return Err(ExitCode::from(1));
    }
    status(config, &format!("repaired flake.nix to use {source_url}"));
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct ShellLaunch {
    args: Vec<OsString>,
    env: Vec<(String, OsString)>,
}

impl ShellLaunch {
    fn args(args: Vec<OsString>) -> Self {
        Self { args, env: vec![] }
    }
}

fn normalize_shell_args(mut args: Vec<OsString>) -> ShellLaunch {
    if args.is_empty() {
        return default_interactive_shell_args();
    }

    if args.len() != 2 || args[0] != "-c" {
        return ShellLaunch::args(args);
    }

    if !args[1].to_string_lossy().chars().any(char::is_whitespace) {
        return ShellLaunch::args(args);
    }

    let mut normalized = Vec::with_capacity(4);
    normalized.push(OsString::from("-c"));
    normalized.push(OsString::from("bash"));
    normalized.push(OsString::from("-lc"));
    normalized.push(args.swap_remove(1));
    ShellLaunch::args(normalized)
}

fn default_interactive_shell_args() -> ShellLaunch {
    let Some(shell) = default_interactive_shell() else {
        return ShellLaunch::args(vec![]);
    };
    shell_args_for(shell.to_string_lossy().as_ref())
}

fn default_interactive_shell() -> Option<PathBuf> {
    if let Some(shell) = env::var_os("ROBO_NIX_SHELL").map(PathBuf::from) {
        return Some(shell);
    }
    if let Some(shell) = parent_interactive_shell() {
        return Some(shell);
    }
    if let Some(shell) = env::var_os("SHELL").map(PathBuf::from) {
        if is_nix_bash(&shell) {
            return login_shell().filter(|login| login.is_file()).or(Some(shell));
        }
        return Some(shell);
    }
    login_shell()
}

fn parent_interactive_shell() -> Option<PathBuf> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    let ppid = after_comm.split_whitespace().nth(1)?;
    let shell = fs::read_link(format!("/proc/{ppid}/exe")).ok()?;
    shell.is_file().then_some(shell)
}

fn is_nix_bash(shell: &Path) -> bool {
    shell.to_string_lossy().contains("/nix/store/")
        && shell.file_name().is_some_and(|name| name == "bash")
}

fn login_shell() -> Option<PathBuf> {
    let user = env::var("USER").ok()?;
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != user {
            return None;
        }
        fields.nth(5).map(PathBuf::from)
    })
}

fn shell_args_for(shell: &str) -> ShellLaunch {
    if shell.is_empty() || !Path::new(shell).is_file() {
        return ShellLaunch::args(vec![]);
    }

    ShellLaunch {
        args: vec![
            OsString::from("-c"),
            OsString::from(shell),
            OsString::from("-i"),
        ],
        env: vec![
            ("SHELL".to_string(), OsString::from(shell)),
            (
                "ROBO_NIX_ACTIVATION_SHELL".to_string(),
                OsString::from(shell),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn shell_command_with_single_quoted_string_uses_shell() {
        let args = vec![OsString::from("-c"), OsString::from("python test.py")];
        let normalized = normalize_shell_args(args);
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "bash", "-lc", "python test.py"]);
    }

    #[test]
    fn shell_command_without_args_uses_user_shell() {
        let normalized = shell_args_for("/bin/sh");
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "/bin/sh", "-i"]);
        assert_eq!(
            normalized.env,
            vec![
                ("SHELL".to_string(), OsString::from("/bin/sh")),
                (
                    "ROBO_NIX_ACTIVATION_SHELL".to_string(),
                    OsString::from("/bin/sh")
                )
            ]
        );
    }

    #[test]
    fn shell_command_keeps_split_argv_intact() {
        let args = vec![
            OsString::from("-c"),
            OsString::from("python"),
            OsString::from("test.py"),
        ];
        let normalized = normalize_shell_args(args.clone());
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "python", "test.py"]);
        assert_eq!(normalized, ShellLaunch::args(args));
    }
}
