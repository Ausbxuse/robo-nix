use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use crate::ui::{self, error, hint, output_with_tree, Config};

#[derive(Debug)]
pub(crate) struct RuntimeInputState {
    key: String,
    files: Vec<(String, String)>,
}

pub(crate) fn run(args: Vec<OsString>, config: Config) -> ExitCode {
    // NOTE: prompt hooks must not break the user's interactive shell.
    if let Err(error) = try_run(args, config) {
        print_refresh_error(config, &error);
    }
    ExitCode::SUCCESS
}

pub(crate) fn runtime_input_state(root: &Path) -> RuntimeInputState {
    let files = runtime_input_fingerprints(root);
    RuntimeInputState {
        key: runtime_input_key(&files),
        files,
    }
}

pub(crate) fn set_active_shell_env(
    command: &mut Command,
    workspace: &Path,
    state: &RuntimeInputState,
) {
    command.env("ROBO_NIX_ACTIVE", "1");
    command.env("ROBO_NIX_ENV_NAME", "robo");
    command.env("WORKSPACE_ROOT", workspace);
    command.env("ROBO_NIX_RUNTIME_INPUT_KEY", &state.key);
    command.env(
        "ROBO_NIX_RUNTIME_INPUT_FILES",
        serialize_runtime_input_files(&state.files),
    );
}

fn try_run(args: Vec<OsString>, config: Config) -> Result<(), RefreshError> {
    if env::var_os("ROBO_NIX_ACTIVE").is_none() {
        return Ok(());
    }

    let shell = args.first().and_then(|arg| arg.to_str()).unwrap_or("sh");
    let workspace =
        env::var_os("WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or(env::current_dir().map_err(|err| {
                RefreshError::new(format!("failed to determine shell workspace: {err}"))
            })?);
    if !workspace.join("robo.nix").exists() {
        hint(
            config,
            &format!(
                "runtime freshness could not be checked outside {}",
                workspace.display()
            ),
        );
        return Ok(());
    }

    let current = runtime_input_state(&workspace);
    if env::var("ROBO_NIX_RUNTIME_INPUT_KEY").ok().as_deref() == Some(current.key.as_str()) {
        return Ok(());
    }

    let changed = changed_runtime_inputs(&workspace, &current);
    print_runtime_refresh_notice(config, &workspace, &changed);
    let mut envs = refreshed_shell_env(&workspace, config)?;
    append_active_shell_env(&mut envs, &workspace, &current);
    print_shell_exports(shell, &envs);
    Ok(())
}

fn refreshed_shell_env(
    workspace: &Path,
    config: Config,
) -> Result<Vec<(String, String)>, RefreshError> {
    // NOTE: stdout from `robo __shell-refresh` is eval'd by the shell hooks.
    // Keep diagnostics on stderr and reserve stdout for export statements.
    let mut command = Command::new("nix");
    command
        .current_dir(workspace)
        .arg("develop")
        .arg("--accept-flake-config")
        .arg("--command")
        .arg("env")
        .arg("-0");
    let output = output_with_tree(
        config,
        &mut command,
        "robo shell",
        "shell: evaluating and realizing dev shell",
    )
    .map_err(|err| {
        RefreshError::new(format!("failed to refresh shell environment: {err}"))
            .with_hint("the current shell remains usable, but may be stale.")
    })?;

    if output.status.success() {
        return parse_env_zero(&output.stdout);
    }

    write_command_output_to_stderr(&output)?;
    Err(RefreshError::new(format!(
        "failed to refresh shell environment; nix develop exited with {}",
        output.status
    ))
    .with_hint("the current shell remains usable, but may be stale."))
}

fn runtime_input_fingerprints(root: &Path) -> Vec<(String, String)> {
    [
        "flake.nix",
        "flake.lock",
        ".python-version",
        "pyproject.toml",
        "uv.lock",
        "robo.nix",
    ]
    .into_iter()
    .map(|path| (path.to_string(), fingerprint_file(&root.join(path))))
    .collect()
}

fn fingerprint_file(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| fingerprint_bytes(&bytes))
        .unwrap_or_else(|_| "missing".to_string())
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn runtime_input_key(files: &[(String, String)]) -> String {
    let mut hasher = DefaultHasher::new();
    "runtime-input-v1".hash(&mut hasher);
    files.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn serialize_runtime_input_files(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(path, hash)| format!("{path}={hash}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_runtime_input_files(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter_map(|entry| entry.split_once('='))
        .map(|(path, hash)| (path.to_string(), hash.to_string()))
        .collect()
}

fn changed_runtime_inputs(root: &Path, current: &RuntimeInputState) -> Vec<PathBuf> {
    let Ok(active_files) = env::var("ROBO_NIX_RUNTIME_INPUT_FILES") else {
        return Vec::new();
    };
    let active_files: BTreeMap<_, _> = parse_runtime_input_files(&active_files)
        .into_iter()
        .collect();

    current
        .files
        .iter()
        .filter_map(|(path, hash)| {
            active_files
                .get(path)
                .is_none_or(|active_hash| active_hash != hash)
                .then(|| root.join(path))
        })
        .collect()
}

fn append_active_shell_env(
    envs: &mut Vec<(String, String)>,
    workspace: &Path,
    state: &RuntimeInputState,
) {
    set_env_value(envs, "ROBO_NIX_ACTIVE", "1".to_string());
    set_env_value(envs, "ROBO_NIX_ENV_NAME", "robo".to_string());
    set_env_value(envs, "ROBO_NIX_PROMPT_PREFIX", "1".to_string());
    set_env_value(envs, "WORKSPACE_ROOT", workspace.display().to_string());
    set_env_value(envs, "ROBO_NIX_RUNTIME_INPUT_KEY", state.key.clone());
    set_env_value(
        envs,
        "ROBO_NIX_RUNTIME_INPUT_FILES",
        serialize_runtime_input_files(&state.files),
    );
}

fn set_env_value(envs: &mut Vec<(String, String)>, name: &str, value: String) {
    envs.retain(|(candidate, _)| candidate != name);
    envs.push((name.to_string(), value));
}

fn parse_env_zero(bytes: &[u8]) -> Result<Vec<(String, String)>, RefreshError> {
    let mut envs = Vec::new();
    for entry in bytes.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let Some(eq) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let name = String::from_utf8(entry[..eq].to_vec()).map_err(|_| {
            RefreshError::new("refreshed shell environment contains an invalid variable name")
        })?;
        let value = String::from_utf8(entry[eq + 1..].to_vec()).map_err(|_| {
            RefreshError::new("refreshed shell environment contains an invalid variable value")
        })?;
        envs.push((name, value));
    }
    Ok(envs)
}

fn write_command_output_to_stderr(output: &Output) -> Result<(), RefreshError> {
    let mut stderr = io::stderr();
    if !output.stdout.is_empty() {
        stderr
            .write_all(b"--- shell refresh stdout ---\n")
            .map_err(|err| RefreshError::new(format!("failed to write refresh stdout: {err}")))?;
        stderr
            .write_all(&output.stdout)
            .map_err(|err| RefreshError::new(format!("failed to write refresh stdout: {err}")))?;
        stderr
            .write_all(b"\n")
            .map_err(|err| RefreshError::new(format!("failed to write refresh stdout: {err}")))?;
    }
    if !output.stderr.is_empty() {
        stderr
            .write_all(b"--- shell refresh stderr ---\n")
            .map_err(|err| RefreshError::new(format!("failed to write refresh stderr: {err}")))?;
        stderr
            .write_all(&output.stderr)
            .map_err(|err| RefreshError::new(format!("failed to write refresh stderr: {err}")))?;
    }
    Ok(())
}

fn print_runtime_refresh_notice(config: Config, workspace: &Path, changed: &[PathBuf]) {
    eprintln!(
        "{}",
        ui::label(
            config,
            &format!("runtime changed in {}", workspace.display()),
            ui::LabelKind::Status
        )
    );
    for path in changed {
        eprintln!(
            "  {} {}",
            ui::label(config, "changed", ui::LabelKind::Hint),
            path.display()
        );
    }
}

fn print_shell_exports(shell: &str, envs: &[(String, String)]) {
    for (name, value) in envs {
        if !is_shell_identifier(name) {
            continue;
        }
        if shell == "fish" {
            println!("set -gx {name} {}", shell_quote(value));
        } else {
            println!("export {name}={}", shell_quote(value));
        }
    }
}

fn is_shell_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Debug)]
struct RefreshError {
    message: String,
    hint: Option<String>,
}

impl RefreshError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

fn print_refresh_error(config: Config, refresh_error: &RefreshError) {
    error(config, &refresh_error.message);
    if let Some(message) = &refresh_error.hint {
        hint(config, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_input_key_tracks_project_file_changes() {
        let root = temp_project("runtime-key");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();

        let before = runtime_input_state(&root);
        fs::write(
            root.join("pyproject.toml"),
            "[project]\ndependencies = []\n",
        )
        .unwrap();
        let after = runtime_input_state(&root);

        assert_ne!(before.key, after.key);
        assert_eq!(
            parse_runtime_input_files(&serialize_runtime_input_files(&after.files)),
            after.files
        );

        cleanup(root);
    }

    #[test]
    fn refreshed_env_overwrites_runtime_state() {
        let root = PathBuf::from("/workspace/project");
        let state = RuntimeInputState {
            key: "new-key".to_string(),
            files: vec![("robo.nix".to_string(), "hash".to_string())],
        };
        let mut envs = vec![
            (
                "ROBO_NIX_RUNTIME_INPUT_KEY".to_string(),
                "old-key".to_string(),
            ),
            ("KEEP_ME".to_string(), "1".to_string()),
        ];

        append_active_shell_env(&mut envs, &root, &state);

        assert_eq!(
            env_value(&envs, "ROBO_NIX_RUNTIME_INPUT_KEY"),
            Some("new-key")
        );
        assert_eq!(env_value(&envs, "ROBO_NIX_ACTIVE"), Some("1"));
        assert_eq!(
            env_value(&envs, "WORKSPACE_ROOT"),
            Some("/workspace/project")
        );
        assert_eq!(env_value(&envs, "KEEP_ME"), Some("1"));
    }

    #[test]
    fn parses_nul_separated_shell_environment() {
        let envs = parse_env_zero(b"PATH=/bin\0BAD\0QUOTE=a'b\0").unwrap();

        assert_eq!(
            envs,
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("QUOTE".to_string(), "a'b".to_string()),
            ]
        );
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
        assert!(is_shell_identifier("ROBO_NIX_ACTIVE"));
        assert!(!is_shell_identifier("not-valid-name"));
    }

    fn env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a str> {
        envs.iter()
            .find_map(|(candidate, value)| (candidate == name).then_some(value.as_str()))
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "robo-minimal-refresh-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
