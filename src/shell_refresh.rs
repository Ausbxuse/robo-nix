use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use crate::env_vars::{is_robo_managed_env, runtime_key_env_names};
use crate::host_cuda::append_host_cuda_driver_bridge;
use crate::nix_env::{
    add_env_capture_args, inherit_terminal_environment, missing_store_roots, parse_env_zero,
};
use crate::ui::{error, hint, output_with_tree, row_err, status, Config};

const OBSERVED_RUNTIME_INPUTS_FILE: &str = "runtime-inputs-v1";

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
    let files = runtime_input_fingerprints(root, |name| env::var(name).ok());
    RuntimeInputState {
        key: runtime_input_key(&files),
        files,
    }
}

pub(crate) fn runtime_input_state_for_env(
    root: &Path,
    envs: &[(String, String)],
) -> RuntimeInputState {
    let files = runtime_input_fingerprints(root, |name| {
        envs.iter()
            .find_map(|(candidate, value)| (candidate == name).then_some(value.clone()))
    });
    RuntimeInputState {
        key: runtime_input_key(&files),
        files,
    }
}

impl RuntimeInputState {
    pub(crate) fn key(&self) -> &str {
        &self.key
    }
}

pub(crate) fn set_active_shell_env(
    command: &mut Command,
    workspace: &Path,
    state: &RuntimeInputState,
    runtime_env: &[(String, String)],
) {
    command.env("ROBO_NIX_ACTIVE", "1");
    command.env("ROBO_NIX_ENV_NAME", "robo");
    command.env("WORKSPACE_ROOT", workspace);
    command.env("ROBO_NIX_RUNTIME_INPUT_KEY", &state.key);
    command.env(
        "ROBO_NIX_RUNTIME_INPUT_FILES",
        serialize_runtime_input_files(&state.files),
    );
    command.env(
        "ROBO_NIX_MANAGED_ENV_VARS",
        managed_env_var_names_from_command_env(state, workspace, runtime_env),
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
    let missing_store_paths = missing_active_store_roots();
    if env::var("ROBO_NIX_RUNTIME_INPUT_KEY").ok().as_deref() == Some(current.key.as_str())
        && missing_store_paths.is_empty()
    {
        return Ok(());
    }

    let changed = changed_runtime_inputs(&workspace, &current);
    print_runtime_refresh_notice(config, &workspace, &changed, &missing_store_paths);
    let mut envs = refreshed_shell_env(&workspace, config)?;
    let _ = append_host_cuda_driver_bridge(&mut envs, &workspace);
    append_refreshed_active_shell_env(&mut envs, &workspace);
    print_shell_delta(shell, &envs);
    Ok(())
}

fn missing_active_store_roots() -> Vec<PathBuf> {
    let names = previous_managed_env_var_names();
    missing_managed_store_roots(&names, |name| env::var(name).ok())
}

fn missing_managed_store_roots<F>(names: &[String], mut env_value: F) -> Vec<PathBuf>
where
    F: FnMut(&str) -> Option<String>,
{
    let envs: Vec<(String, String)> = names
        .iter()
        .filter_map(|name| env_value(name).map(|value| (name.clone(), value)))
        .collect();
    missing_store_roots(&envs)
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
        .arg("--log-format")
        .arg("raw")
        .arg("develop")
        .arg("--impure")
        .arg("--accept-flake-config")
        .arg("--command");
    add_env_capture_args(&mut command);
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
        let _ = record_observed_runtime_inputs(&workspace, &output.stderr);
        let mut envs = parse_env_zero(&output.stdout).map_err(RefreshError::new)?;
        inherit_terminal_environment(&mut envs);
        return Ok(envs);
    }

    write_command_output_to_stderr(&output)?;
    Err(RefreshError::new(format!(
        "failed to refresh shell environment; nix develop exited with {}",
        output.status
    ))
    .with_hint("the current shell remains usable, but may be stale."))
}

fn runtime_input_fingerprints<F>(root: &Path, mut env_value: F) -> Vec<(String, String)>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut runtime_files = [
        "flake.nix",
        "flake.lock",
        ".python-version",
        "pyproject.toml",
        "uv.lock",
        "robo.nix",
        ".venv/bin/python",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    runtime_files.extend(local_nix_runtime_inputs(root));
    runtime_files.extend(read_observed_runtime_inputs(root));

    let mut files = runtime_files
        .into_iter()
        .map(|path| {
            let fingerprint = fingerprint_file(&root.join(&path));
            (path, fingerprint)
        })
        .collect::<Vec<_>>();
    files.extend(runtime_key_env_names().map(|name| {
        (
            format!("env:{name}"),
            env_value(name).unwrap_or_else(|| "unset".to_string()),
        )
    }));
    files
}

pub(crate) fn record_observed_runtime_inputs(root: &Path, nix_stderr: &[u8]) -> io::Result<()> {
    let inputs = observed_runtime_inputs_from_nix_stderr(root, nix_stderr);
    if inputs.is_empty() {
        return Ok(());
    }
    write_observed_runtime_inputs(root, &inputs)
}

fn read_observed_runtime_inputs(root: &Path) -> BTreeSet<String> {
    fs::read_to_string(observed_runtime_inputs_path(root))
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter(|line| !line.starts_with("env:"))
                .filter(|line| is_safe_observed_runtime_input(line))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn write_observed_runtime_inputs(root: &Path, inputs: &BTreeSet<String>) -> io::Result<()> {
    let state_dir = root.join(".robo-nix");
    fs::create_dir_all(&state_dir)?;
    let path = observed_runtime_inputs_path(root);
    let tmp_path = state_dir.join(format!(
        "{OBSERVED_RUNTIME_INPUTS_FILE}.tmp-{}",
        std::process::id()
    ));
    let mut text = inputs.iter().cloned().collect::<Vec<_>>().join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    fs::write(&tmp_path, text)?;
    fs::rename(tmp_path, path)
}

fn observed_runtime_inputs_path(root: &Path) -> PathBuf {
    root.join(".robo-nix").join(OBSERVED_RUNTIME_INPUTS_FILE)
}

fn observed_runtime_inputs_from_nix_stderr(root: &Path, bytes: &[u8]) -> BTreeSet<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(nix_evaluated_file_path)
        .filter_map(|path| observed_workspace_nix_path(root, Path::new(path)))
        .collect()
}

fn nix_evaluated_file_path(line: &str) -> Option<&str> {
    line.rsplit('\r')
        .next()
        .unwrap_or(line)
        .trim()
        .strip_prefix("evaluating file '")?
        .strip_suffix("'")
}

fn observed_workspace_nix_path(root: &Path, path: &Path) -> Option<String> {
    if path.extension().is_none_or(|extension| extension != "nix") {
        return None;
    }
    if let Some(relative) = workspace_relative_path(root, path) {
        return Some(relative);
    }
    store_source_relative_path(path).and_then(|relative| {
        let workspace_path = root.join(&relative);
        files_match(path, &workspace_path).then_some(relative)
    })
}

fn store_source_relative_path(path: &Path) -> Option<String> {
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (
            Some(Component::RootDir),
            Some(Component::Normal(nix)),
            Some(Component::Normal(store)),
        ) if nix == "nix" && store == "store" => {}
        _ => return None,
    }
    components.next()?;
    let relative = components.as_path();
    let text = relative.to_string_lossy();
    (!text.is_empty() && is_safe_observed_runtime_input(&text)).then(|| text.to_string())
}

fn is_safe_observed_runtime_input(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path.extension().is_some_and(|extension| extension == "nix")
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn files_match(left: &Path, right: &Path) -> bool {
    let Ok(left) = fs::read(left) else {
        return false;
    };
    let Ok(right) = fs::read(right) else {
        return false;
    };
    left == right
}

fn local_nix_runtime_inputs(root: &Path) -> BTreeSet<String> {
    let mut tracked = BTreeSet::new();
    let mut scanned = BTreeSet::new();
    let mut queue = VecDeque::from([PathBuf::from("flake.nix"), PathBuf::from("robo.nix")]);

    while let Some(relative_file) = queue.pop_front() {
        let relative_file = normalize_relative_path(relative_file);
        if !scanned.insert(relative_file.clone()) {
            continue;
        }

        let absolute_file = root.join(&relative_file);
        let Ok(contents) = fs::read_to_string(&absolute_file) else {
            continue;
        };
        let base = absolute_file.parent().unwrap_or(root);

        for literal in nix_relative_path_literals(&contents) {
            let Some(candidate) = nix_runtime_input_candidate(base, &literal) else {
                continue;
            };
            let Some(relative_candidate) = workspace_relative_path(root, &candidate) else {
                continue;
            };
            tracked.insert(relative_candidate.clone());
            queue.push_back(PathBuf::from(relative_candidate));
        }
    }

    tracked
}

fn nix_runtime_input_candidate(base: &Path, literal: &str) -> Option<PathBuf> {
    if literal.contains("${") {
        return None;
    }
    let path = normalize_path(base.join(literal));
    if literal.ends_with(".nix") || path.extension().is_some_and(|extension| extension == "nix") {
        return Some(path);
    }
    if path.extension().is_none() {
        return Some(path.join("default.nix"));
    }
    None
}

fn workspace_relative_path(root: &Path, path: &Path) -> Option<String> {
    let root = normalize_path(root.to_path_buf());
    let path = normalize_path(path.to_path_buf());
    let relative = path.strip_prefix(root).ok()?;
    let text = relative.to_string_lossy();
    (!text.is_empty()).then(|| text.to_string())
}

fn normalize_relative_path(path: PathBuf) -> PathBuf {
    let normalized = normalize_path(path);
    normalized
        .strip_prefix("/")
        .map(Path::to_path_buf)
        .unwrap_or(normalized)
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn nix_relative_path_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut literals = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'#' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'"' => index = skip_double_quoted_string(bytes, index),
            b'\'' if bytes.get(index + 1) == Some(&b'\'') => {
                index = skip_indented_string(bytes, index)
            }
            b'.' if bytes.get(index + 1) == Some(&b'/') && path_literal_boundary(bytes, index) => {
                let (literal, next) = read_path_literal(bytes, index);
                literals.push(literal);
                index = next;
            }
            b'.' if bytes.get(index + 1) == Some(&b'.')
                && bytes.get(index + 2) == Some(&b'/')
                && path_literal_boundary(bytes, index) =>
            {
                let (literal, next) = read_path_literal(bytes, index);
                literals.push(literal);
                index = next;
            }
            _ => index += 1,
        }
    }

    literals
}

fn skip_double_quoted_string(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    index
}

fn skip_indented_string(bytes: &[u8], mut index: usize) -> usize {
    index += 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\'' && bytes[index + 1] == b'\'' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn path_literal_boundary(bytes: &[u8], index: usize) -> bool {
    index == 0 || !is_path_literal_char(bytes[index - 1])
}

fn read_path_literal(bytes: &[u8], mut index: usize) -> (String, usize) {
    let start = index;
    while index < bytes.len() && is_path_literal_char(bytes[index]) {
        index += 1;
    }
    (
        String::from_utf8_lossy(&bytes[start..index]).to_string(),
        index,
    )
}

fn is_path_literal_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'_' | b'+' | b'-')
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

fn changed_runtime_inputs(_root: &Path, current: &RuntimeInputState) -> Vec<String> {
    let Ok(active_files) = env::var("ROBO_NIX_RUNTIME_INPUT_FILES") else {
        return Vec::new();
    };
    let active_files: BTreeMap<_, _> = parse_runtime_input_files(&active_files)
        .into_iter()
        .collect();
    let current_files: BTreeMap<_, _> = current.files.iter().cloned().collect();

    let mut changed = current
        .files
        .iter()
        .filter_map(|(path, hash)| {
            active_files
                .get(path)
                .is_none_or(|active_hash| active_hash != hash)
                .then(|| path.clone())
        })
        .collect::<BTreeSet<_>>();

    changed.extend(
        active_files
            .keys()
            .filter(|path| !current_files.contains_key(*path))
            .cloned(),
    );

    changed.into_iter().collect()
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
    set_env_value(
        envs,
        "ROBO_NIX_MANAGED_ENV_VARS",
        managed_env_var_names(envs),
    );
}

fn append_refreshed_active_shell_env(envs: &mut Vec<(String, String)>, workspace: &Path) {
    let state = runtime_input_state_for_env(workspace, envs);
    append_active_shell_env(envs, workspace, &state);
}

fn set_env_value(envs: &mut Vec<(String, String)>, name: &str, value: String) {
    envs.retain(|(candidate, _)| candidate != name);
    envs.push((name.to_string(), value));
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

fn print_runtime_refresh_notice(
    config: Config,
    workspace: &Path,
    changed: &[String],
    missing_store_paths: &[PathBuf],
) {
    let reason = if changed.is_empty() {
        "runtime store paths disappeared"
    } else if missing_store_paths.is_empty() {
        "runtime inputs changed"
    } else {
        "runtime inputs changed and store paths disappeared"
    };
    status(
        config,
        &format!("shell: {reason} in {}", workspace.display()),
    );
    for path in changed {
        row_err(
            config,
            "!",
            "changed",
            &display_runtime_input_name(workspace, path),
        );
    }
    for path in missing_store_paths {
        row_err(config, "!", "missing", &path.display().to_string());
    }
}

fn display_runtime_input_name(workspace: &Path, name: &str) -> String {
    if name.starts_with("env:") {
        return name.to_string();
    }
    display_runtime_input_path(workspace, &workspace.join(name))
}

fn display_runtime_input_path(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .map(|path| format!("./{}", path.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

fn print_shell_delta(shell: &str, envs: &[(String, String)]) {
    for line in shell_delta_lines(shell, envs, &previous_managed_env_var_names()) {
        println!("{line}");
    }
}

fn shell_delta_lines(
    shell: &str,
    envs: &[(String, String)],
    previous_names: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    let new_names: BTreeMap<_, _> = envs.iter().map(|(name, _)| (name.as_str(), ())).collect();
    for name in previous_names {
        if !new_names.contains_key(name.as_str()) && is_shell_identifier(&name) {
            if shell == "fish" {
                lines.push(format!("set -e {name}"));
            } else {
                lines.push(format!("unset {name}"));
            }
        }
    }

    for (name, value) in envs {
        if !is_shell_identifier(name) {
            continue;
        }
        if shell == "fish" {
            lines.push(format!("set -gx {name} {}", shell_quote(value)));
        } else {
            lines.push(format!("export {name}={}", shell_quote(value)));
        }
    }
    lines
}

fn previous_managed_env_var_names() -> Vec<String> {
    env::var("ROBO_NIX_MANAGED_ENV_VARS")
        .unwrap_or_default()
        .split(':')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

fn managed_env_var_names(envs: &[(String, String)]) -> String {
    let mut names = envs
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| is_robo_managed_env(name))
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names.join(":")
}

fn managed_env_var_names_from_command_env(
    state: &RuntimeInputState,
    workspace: &Path,
    runtime_env: &[(String, String)],
) -> String {
    let mut envs = runtime_env.to_vec();
    append_active_shell_env(&mut envs, workspace, state);
    managed_env_var_names(&envs)
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
    use std::sync::{Mutex, OnceLock};

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
    fn runtime_input_key_tracks_local_nix_import_changes() {
        let root = temp_project("runtime-key-import");
        fs::create_dir_all(root.join("nix")).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            r#"
let
  runtimeLibs = import ./nix/runtime-libs.nix;
in
{
  components = [ "python-uv" ];
  extraRuntimeLibraries = pkgs: runtimeLibs pkgs;
}
"#,
        )
        .unwrap();
        fs::write(root.join("nix/runtime-libs.nix"), "pkgs: [ pkgs.assimp ]\n").unwrap();

        let before = runtime_input_state(&root);
        fs::write(
            root.join("nix/runtime-libs.nix"),
            "pkgs: [ pkgs.assimp pkgs.glfw ]\n",
        )
        .unwrap();
        let after = runtime_input_state(&root);

        assert_ne!(before.key, after.key);
        assert!(after
            .files
            .iter()
            .any(|(path, _)| path == "nix/runtime-libs.nix"));

        cleanup(root);
    }

    #[test]
    fn runtime_input_key_tracks_local_nix_default_imports() {
        let root = temp_project("runtime-key-default-import");
        fs::create_dir_all(root.join("nix/runtime")).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            r#"
{
  components = import ./nix/runtime;
}
"#,
        )
        .unwrap();
        fs::write(root.join("nix/runtime/default.nix"), "[ \"python-uv\" ]\n").unwrap();

        let state = runtime_input_state(&root);

        assert!(state
            .files
            .iter()
            .any(|(path, _)| path == "nix/runtime/default.nix"));

        cleanup(root);
    }

    #[test]
    fn runtime_input_key_tracks_observed_nix_files() {
        let root = temp_project("runtime-key-observed");
        fs::create_dir_all(root.join("nix")).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();
        fs::write(root.join("nix/runtime-libs.nix"), "pkgs: [ pkgs.assimp ]\n").unwrap();
        let stderr = format!(
            "evaluating file '{}'\n",
            root.join("nix/runtime-libs.nix").display()
        );

        record_observed_runtime_inputs(&root, stderr.as_bytes()).unwrap();
        let before = runtime_input_state(&root);
        fs::write(
            root.join("nix/runtime-libs.nix"),
            "pkgs: [ pkgs.assimp pkgs.glfw ]\n",
        )
        .unwrap();
        let after = runtime_input_state(&root);

        assert_ne!(before.key, after.key);
        assert!(after
            .files
            .iter()
            .any(|(path, _)| path == "nix/runtime-libs.nix"));

        cleanup(root);
    }

    #[test]
    fn observed_nix_inputs_ignore_unsafe_state_paths() {
        let root = temp_project("runtime-key-observed-safe");
        fs::create_dir_all(root.join(".robo-nix")).unwrap();
        fs::write(
            observed_runtime_inputs_path(&root),
            "nix/runtime-libs.nix\n../outside.nix\n/tmp/absolute.nix\nenv:LD_LIBRARY_PATH\nnix/text.txt\n",
        )
        .unwrap();

        assert_eq!(
            read_observed_runtime_inputs(&root),
            BTreeSet::from(["nix/runtime-libs.nix".to_string()])
        );

        cleanup(root);
    }

    #[test]
    fn store_source_observed_paths_map_back_to_workspace_inputs() {
        assert_eq!(
            store_source_relative_path(Path::new("/nix/store/abc-source/nix/runtime-libs.nix")),
            Some("nix/runtime-libs.nix".to_string())
        );
        assert_eq!(
            store_source_relative_path(Path::new("/nix/store/abc-source/../bad.nix")),
            None
        );
    }

    #[test]
    fn empty_observed_input_capture_does_not_clear_previous_state() {
        let root = temp_project("runtime-key-observed-empty");
        fs::create_dir_all(root.join(".robo-nix")).unwrap();
        fs::write(
            observed_runtime_inputs_path(&root),
            "nix/runtime-libs.nix\n",
        )
        .unwrap();

        record_observed_runtime_inputs(&root, b"").unwrap();

        assert_eq!(
            read_observed_runtime_inputs(&root),
            BTreeSet::from(["nix/runtime-libs.nix".to_string()])
        );

        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn failed_refresh_does_not_poison_next_prompt_retry() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = env_lock().lock().unwrap();
        let root = temp_project("failed-refresh-retry");
        fs::create_dir_all(root.join("nix")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(root.join("robo.nix"), "import ./nix/runtime-libs.nix\n").unwrap();
        fs::write(
            root.join("nix/runtime-libs.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();
        let active_state = runtime_input_state(&root);
        fs::write(
            root.join("nix/runtime-libs.nix"),
            "{ components = [ \"python-uv\" \"native-build\" ]; }\n",
        )
        .unwrap();

        let fake_nix = root.join("bin").join("nix");
        let count_path = root.join("nix-count");
        fs::write(
            &fake_nix,
            r#"#!/bin/sh
count_file="${ROBO_NIX_FAKE_NIX_COUNT:?}"
if [ -f "$count_file" ]; then
  count="$(cat "$count_file")"
else
  count=0
fi
count=$((count + 1))
printf '%s\n' "$count" > "$count_file"
if [ "$count" -eq 1 ]; then
  printf '%s\n' 'forced nix failure' >&2
  exit 1
fi
printf '\000robo-nix-env-start\000PATH=/bin\000ROBO_NIX_COMPONENTS=python-uv\000'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_nix).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_nix, permissions).unwrap();

        let path = env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![root.join("bin")];
        paths.extend(env::split_paths(&path));
        let path = env::join_paths(paths).unwrap();
        let _env = EnvGuard::set(&[
            ("PATH", path),
            ("ROBO_NIX_ACTIVE", OsString::from("1")),
            ("WORKSPACE_ROOT", root.clone().into_os_string()),
            (
                "ROBO_NIX_RUNTIME_INPUT_KEY",
                OsString::from(active_state.key.clone()),
            ),
            (
                "ROBO_NIX_RUNTIME_INPUT_FILES",
                OsString::from(serialize_runtime_input_files(&active_state.files)),
            ),
            ("ROBO_NIX_MANAGED_ENV_VARS", OsString::new()),
            (
                "ROBO_NIX_FAKE_NIX_COUNT",
                count_path.clone().into_os_string(),
            ),
        ]);

        assert!(try_run(vec![OsString::from("sh")], test_config()).is_err());
        assert_eq!(fs::read_to_string(&count_path).unwrap().trim(), "1");

        try_run(vec![OsString::from("sh")], test_config()).unwrap();
        assert_eq!(fs::read_to_string(&count_path).unwrap().trim(), "2");

        cleanup(root);
    }

    #[test]
    fn nix_path_literal_scan_ignores_comments_and_strings() {
        assert_eq!(
            nix_relative_path_literals(
                r#"
import ./nix/runtime-libs.nix
# import ./nix/commented.nix
"./nix/string.nix"
''
  ./nix/indented-string.nix
''
src = ./src;
"#
            ),
            vec!["./nix/runtime-libs.nix", "./src"]
        );
    }

    #[test]
    fn runtime_input_key_can_use_launch_environment() {
        let root = temp_project("runtime-key-env");
        fs::create_dir_all(&root).unwrap();

        let state = runtime_input_state_for_env(
            &root,
            &[(
                "LD_LIBRARY_PATH".to_string(),
                "/nix/store/runtime/lib".to_string(),
            )],
        );

        assert!(state.files.iter().any(|(name, value)| {
            name == "env:LD_LIBRARY_PATH" && value == "/nix/store/runtime/lib"
        }));

        cleanup(root);
    }

    #[test]
    fn missing_managed_store_roots_detects_stale_compiler_paths() {
        let names = vec!["CC".to_string(), "CXX".to_string(), "UNRELATED".to_string()];

        let missing = missing_managed_store_roots(&names, |name| match name {
            "CC" => Some("/nix/store/robo-missing-gcc-wrapper-14.3.0/bin/cc".to_string()),
            "CXX" => Some("/usr/bin/c++".to_string()),
            "UNRELATED" => Some("1".to_string()),
            _ => None,
        });

        assert_eq!(
            missing,
            vec![PathBuf::from(
                "/nix/store/robo-missing-gcc-wrapper-14.3.0/bin/cc"
            )]
        );
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
        assert!(env_value(&envs, "ROBO_NIX_MANAGED_ENV_VARS")
            .unwrap()
            .contains("ROBO_NIX_RUNTIME_INPUT_KEY"));
    }

    #[test]
    fn refreshed_env_records_final_runtime_env_inputs() {
        let root = temp_project("refreshed-final-env");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("robo.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();
        let mut envs = vec![(
            "LD_LIBRARY_PATH".to_string(),
            "/nix/store/final-runtime/lib".to_string(),
        )];

        append_refreshed_active_shell_env(&mut envs, &root);

        let files = env_value(&envs, "ROBO_NIX_RUNTIME_INPUT_FILES").unwrap();
        assert!(files.contains("env:LD_LIBRARY_PATH=/nix/store/final-runtime/lib"));
        assert_eq!(
            env_value(&envs, "ROBO_NIX_RUNTIME_INPUT_KEY"),
            Some(runtime_input_state_for_env(&root, &envs).key.as_str())
        );

        cleanup(root);
    }

    #[test]
    fn refresh_notice_paths_are_workspace_relative() {
        let workspace = Path::new("/workspace/robot");

        assert_eq!(
            display_runtime_input_path(workspace, Path::new("/workspace/robot/pyproject.toml")),
            "./pyproject.toml"
        );
        assert_eq!(
            display_runtime_input_path(workspace, Path::new("/tmp/pyproject.toml")),
            "/tmp/pyproject.toml"
        );
        assert_eq!(
            display_runtime_input_name(workspace, "nix/runtime-libs.nix"),
            "./nix/runtime-libs.nix"
        );
        assert_eq!(
            display_runtime_input_name(workspace, "env:LD_LIBRARY_PATH"),
            "env:LD_LIBRARY_PATH"
        );
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

    #[test]
    fn managed_env_names_exclude_unowned_shell_values() {
        let envs = vec![
            ("ROBO_NIX_ACTIVE".to_string(), "1".to_string()),
            (
                "ROBO_NIX_LIBC_DEV".to_string(),
                "/nix/store/glibc-dev".to_string(),
            ),
            ("ROBO_NIX_SHELL".to_string(), "/bin/zsh".to_string()),
            ("LD_LIBRARY_PATH".to_string(), "/nix/store/lib".to_string()),
            ("UNRELATED".to_string(), "1".to_string()),
        ];

        assert_eq!(
            managed_env_var_names(&envs),
            "LD_LIBRARY_PATH:ROBO_NIX_ACTIVE:ROBO_NIX_LIBC_DEV"
        );
    }

    #[test]
    fn shell_delta_unsets_removed_robo_managed_values() {
        let envs = vec![("ROBO_NIX_ACTIVE".to_string(), "1".to_string())];
        let previous = vec![
            "ROBO_NIX_ACTIVE".to_string(),
            "LD_LIBRARY_PATH".to_string(),
            "not-valid-name".to_string(),
        ];

        assert_eq!(
            shell_delta_lines("zsh", &envs, &previous),
            vec![
                "unset LD_LIBRARY_PATH".to_string(),
                "export ROBO_NIX_ACTIVE='1'".to_string()
            ]
        );
        assert_eq!(
            shell_delta_lines("fish", &envs, &previous)[0],
            "set -e LD_LIBRARY_PATH"
        );
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

    fn test_config() -> Config {
        Config {
            color: false,
            debug: false,
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn set(values: &[(&'static str, OsString)]) -> Self {
            let saved = values
                .iter()
                .map(|(name, _)| (*name, env::var_os(name)))
                .collect::<Vec<_>>();
            for (name, value) in values {
                env::set_var(name, value);
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in self.saved.iter().rev() {
                match value {
                    Some(value) => env::set_var(name, value),
                    None => env::remove_var(name),
                }
            }
        }
    }
}
