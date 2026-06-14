use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::error::AppError;
use crate::profile::RuntimeProfile;
use crate::ui::{
    output_cached_tree, output_with_tree_command_steps, output_with_tree_steps,
    CommandProgressStep, Config, ProgressStep,
};

const ENV_START_MARKER: &[u8] = b"robo-nix-env-start";
const ENV_CAPTURE_SCRIPT: &str = "printf '\\000robo-nix-env-start\\000'; env -0";
const RUNTIME_ENV_CACHE_MAGIC: &str = "robo-nix-runtime-env-cache-v1";
const RUNTIME_ENV_CACHE_FILE: &str = "runtime-env-cache-v1.env0";
const GIT_TRACKED_RUNTIME_INPUTS: &[&str] = &[".python-version", "flake.nix", "robo.nix"];
const ROBO_NIX_EXTRA_SUBSTITUTERS: &str = concat!(
    "https://cache.nixos.org ",
    "https://cache.zhenyuzhao.com/zhenyu-public ",
    "https://nixpkgs-python.cachix.org"
);
const ROBO_NIX_EXTRA_TRUSTED_PUBLIC_KEYS: &str = concat!(
    "cache.nixos.org-1:6NCHdD59X431o0gWmIcT6moLroYd5eH4kW+7CA+PajY= ",
    "zhenyu-public:HJAdTzf9fcFLcjDfAE39xyQv20Ev4M8kTFzXx3ssGho= ",
    "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
);
const ROBO_NIX_CACHE_OPTIONS: [(&str, &str); 3] = [
    ("extra-substituters", ROBO_NIX_EXTRA_SUBSTITUTERS),
    (
        "extra-trusted-public-keys",
        ROBO_NIX_EXTRA_TRUSTED_PUBLIC_KEYS,
    ),
    ("narinfo-cache-negative-ttl", "0"),
];
const INHERITED_TERMINAL_ENV_VARS: &[&str] = &[
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "TERMINFO",
    "TERMINFO_DIRS",
    "TMUX",
    "TMUX_PANE",
    "STY",
];
const TERMINAL_COLOR_ENV_VARS: &[&str] = &["NO_COLOR", "FORCE_COLOR", "CLICOLOR", "CLICOLOR_FORCE"];
const VOLATILE_RUNTIME_ENV_VARS: &[&str] = &["TMPDIR", "TMP", "TEMP", "TEMPDIR", "PWD", "OLDPWD"];

pub(crate) fn nix_command() -> Command {
    let mut command = Command::new("nix");
    remove_volatile_runtime_env(&mut command);
    append_robo_nix_cache_options(&mut command);
    command
}

fn nix_store_command() -> Command {
    let mut command = Command::new("nix-store");
    remove_volatile_runtime_env(&mut command);
    append_robo_nix_cache_options(&mut command);
    command
}

fn remove_volatile_runtime_env(command: &mut Command) {
    for name in VOLATILE_RUNTIME_ENV_VARS {
        command.env_remove(name);
    }
}

fn remove_runtime_library_env(command: &mut Command) {
    command.env_remove("LD_LIBRARY_PATH");
}

pub(crate) fn apply_profile_env(command: &mut Command, profile: &RuntimeProfile) {
    if let Some(profile) = profile.requested() {
        command.env("ROBO_NIX_PROFILE", profile);
    } else {
        command.env_remove("ROBO_NIX_PROFILE");
    }
}

fn append_robo_nix_cache_options(command: &mut Command) -> &mut Command {
    for (name, value) in ROBO_NIX_CACHE_OPTIONS {
        command.arg("--option").arg(name).arg(value);
    }
    command
}

pub(crate) fn runtime_environment(
    config: Config,
    phase: &str,
    workspace: &Path,
    profile: &RuntimeProfile,
    cache_key: &str,
) -> Result<Vec<(String, String)>, AppError> {
    let cache = read_runtime_env_cache(workspace, profile, cache_key);
    match cache {
        RuntimeEnvCache::Hit(mut envs) => {
            output_cached_tree(config, "runtime cache");
            inherit_terminal_environment(&mut envs);
            return Ok(envs);
        }
        RuntimeEnvCache::Miss(reason) => {
            if config.debug {
                crate::ui::debug(config, &format!("runtime cache {}", reason.detail()));
            }
            if config.debug {
                if let Some(estimate) = estimate_runtime_disk_size(workspace, profile) {
                    crate::ui::status(config, &estimate.status_line(phase));
                }
            }

            let flake_ref = workspace_flake_ref(workspace);
            let mut prefetch_command = runtime_prefetch_command(workspace, profile);
            let mut command = nix_command();
            apply_profile_env(&mut command, profile);
            command
                .current_dir(workspace)
                .arg("--log-format")
                .arg("raw")
                .arg("develop")
                .arg(&flake_ref)
                .arg("--impure")
                .arg("--accept-flake-config")
                .arg("--command")
                .arg("sh")
                .arg("-c")
                .arg(ENV_CAPTURE_SCRIPT);
            let completed_steps = vec![ProgressStep::instant("runtime cache", reason.label())];
            let output = if let Some(prefetch_command) = prefetch_command.as_mut() {
                let mut steps = [
                    CommandProgressStep {
                        message: "prefetching runtime paths",
                        command: prefetch_command,
                        success_suffix: None,
                        required: false,
                    },
                    CommandProgressStep {
                        message: "evaluating runtime shell",
                        command: &mut command,
                        success_suffix: None,
                        required: true,
                    },
                ];
                let mut outputs = output_with_tree_command_steps(
                    config,
                    &format!("robo {phase}"),
                    completed_steps,
                    &mut steps,
                )
                .map_err(|err| {
                    AppError::project(format!("failed to start nix: {err}"))
                        .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
                })?;
                outputs.pop().ok_or_else(|| {
                    AppError::project("nix develop did not produce command output")
                        .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
                })?
            } else {
                output_with_tree_steps(
                    config,
                    &mut command,
                    &format!("robo {phase}"),
                    "evaluating runtime shell",
                    completed_steps,
                )
                .map_err(|err| {
                    AppError::project(format!("failed to start nix: {err}"))
                        .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
                })?
            };

            if output.status.success() {
                let _ =
                    crate::shell_refresh::record_observed_runtime_inputs(workspace, &output.stderr);
                let mut envs = parse_env_zero(&output.stdout).map_err(AppError::project)?;
                remove_volatile_runtime_env_values(&mut envs);
                inherit_terminal_environment(&mut envs);
                return Ok(envs);
            }

            crate::write_command_output(&filter_nix_output_for_user(&output))?;
            Err(AppError::project(format!(
                "nix develop exited with {}",
                output.status
            ))
            .with_hint("review the Nix output above and attach .robo-nix/last-error.log to an issue if this looks like a robo-nix bug."))
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RuntimeDiskEstimate {
    known_bytes: u64,
    known_paths: usize,
    unknown_paths: usize,
}

impl RuntimeDiskEstimate {
    fn status_line(&self, _phase: &str) -> String {
        let mut line = format!(
            "approximate runtime closure {} across {} store paths",
            human_bytes(self.known_bytes),
            self.known_paths
        );
        if self.unknown_paths > 0 {
            line.push_str(&format!("; {} paths not yet sized", self.unknown_paths));
        }
        line
    }
}

fn estimate_runtime_disk_size(
    workspace: &Path,
    profile: &RuntimeProfile,
) -> Option<RuntimeDiskEstimate> {
    let derivation = runtime_shell_derivation(workspace, profile)?;
    let requisites = runtime_shell_requisites(workspace, &derivation)?;
    let mut paths = requisites
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("/nix/store/") && !line.ends_with(".drv"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return None;
    }

    let mut known_bytes = 0;
    let mut known_paths = 0;
    for chunk in paths.chunks(200) {
        let mut command = nix_command();
        command.arg("path-info").arg("--size");
        command.args(chunk);
        let output = command.current_dir(workspace).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (bytes, count) = parse_path_info_size_output(&stdout);
        known_bytes += bytes;
        known_paths += count;
    }

    Some(RuntimeDiskEstimate {
        known_bytes,
        known_paths,
        unknown_paths: paths.len().saturating_sub(known_paths),
    })
}

fn runtime_shell_derivation(workspace: &Path, profile: &RuntimeProfile) -> Option<String> {
    let mut current_system_command = nix_command();
    apply_profile_env(&mut current_system_command, profile);
    current_system_command
        .current_dir(workspace)
        .arg("eval")
        .arg("--impure")
        .arg("--raw")
        .arg("--expr")
        .arg("builtins.currentSystem");
    let current_system = command_stdout(&mut current_system_command)?;
    let current_system = current_system.trim();
    if current_system.is_empty() {
        return None;
    }

    let flake_ref = workspace_flake_ref(workspace);
    let dev_shell_attr = format!("{flake_ref}#devShells.{current_system}.default");
    let mut derivation_command = nix_command();
    apply_profile_env(&mut derivation_command, profile);
    derivation_command
        .current_dir(workspace)
        .arg("path-info")
        .arg("--impure")
        .arg("--accept-flake-config")
        .arg("--derivation")
        .arg(dev_shell_attr);
    let derivation = command_stdout(&mut derivation_command)?;
    let derivation = derivation
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("/nix/store/") && line.ends_with(".drv"))?;
    Some(derivation.to_string())
}

fn runtime_shell_requisites(workspace: &Path, derivation: &str) -> Option<String> {
    let mut requisites_command = nix_store_command();
    requisites_command
        .current_dir(workspace)
        .arg("-q")
        .arg("--requisites")
        .arg("--include-outputs")
        .arg(derivation);
    command_stdout(&mut requisites_command)
}

fn runtime_prefetch_command(workspace: &Path, profile: &RuntimeProfile) -> Option<Command> {
    let executable = env::current_exe().ok()?;
    let mut command = Command::new(executable);
    command.current_dir(workspace).arg("__runtime-prefetch");
    apply_profile_env(&mut command, profile);
    Some(command)
}

pub(crate) fn prefetch_runtime_input_outputs(
    workspace: &Path,
    profile: &RuntimeProfile,
) -> Result<(), AppError> {
    let Some(mut command) = runtime_input_output_prefetch_command(workspace, profile) else {
        return Ok(());
    };
    let _ = command
        .status()
        .map_err(|err| AppError::project(format!("failed to start nix: {err}")))?;
    Ok(())
}

fn runtime_input_output_prefetch_command(
    workspace: &Path,
    profile: &RuntimeProfile,
) -> Option<Command> {
    let Some(derivation) = runtime_shell_derivation(workspace, profile) else {
        return None;
    };
    let Some(installables) = runtime_input_output_installables(workspace, &derivation) else {
        return None;
    };
    if installables.is_empty() {
        return None;
    }

    let mut command = nix_command();
    apply_profile_env(&mut command, profile);
    command
        .current_dir(workspace)
        .arg("--option")
        .arg("max-jobs")
        .arg("0")
        .arg("build")
        .arg("--no-link")
        .arg("--keep-going")
        .args(installables);
    Some(command)
}

fn runtime_input_output_installables(workspace: &Path, derivation: &str) -> Option<Vec<String>> {
    let mut command = nix_command();
    command
        .current_dir(workspace)
        .arg("derivation")
        .arg("show")
        .arg("--no-pretty")
        .arg(derivation);
    let output = command_stdout(&mut command)?;
    input_output_installables_from_derivation_json(&output)
}

fn input_output_installables_from_derivation_json(output: &str) -> Option<Vec<String>> {
    let value: Value = serde_json::from_str(output).ok()?;
    let root = value.as_object()?;
    let derivations = root
        .get("derivations")
        .and_then(Value::as_object)
        .unwrap_or(root);
    let (_path, derivation) = derivations.iter().next()?;
    let drvs = derivation.get("inputs")?.get("drvs")?.as_object()?;
    let mut installables = BTreeSet::new();
    for (drv, input) in drvs {
        let drv_path = if drv.starts_with("/nix/store/") {
            drv.to_string()
        } else {
            format!("/nix/store/{drv}")
        };
        let Some(outputs) = input.get("outputs").and_then(Value::as_array) else {
            continue;
        };
        for output in outputs {
            let Some(output) = output.as_str() else {
                continue;
            };
            installables.insert(format!("{drv_path}^{output}"));
        }
    }
    Some(installables.into_iter().collect())
}

pub(crate) fn workspace_flake_ref(workspace: &Path) -> String {
    workspace_flake_ref_for_git_state(workspace, has_untracked_git_runtime_inputs(workspace))
}

fn workspace_flake_ref_for_git_state(
    workspace: &Path,
    has_untracked_runtime_inputs: bool,
) -> String {
    if has_untracked_runtime_inputs {
        format!("path:{}", workspace.display())
    } else {
        ".".to_string()
    }
}

fn has_untracked_git_runtime_inputs(workspace: &Path) -> bool {
    let mut rev_parse = git_command(workspace);
    let Ok(output) = rev_parse
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let mut ls_files = git_command(workspace);
    let Ok(output) = ls_files
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg("--")
        .args(GIT_TRACKED_RUNTIME_INPUTS)
        .output()
    else {
        return true;
    };
    !output.status.success()
}

fn git_command(workspace: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(workspace);
    remove_runtime_library_env(&mut command);
    command
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_path_info_size_output(output: &str) -> (u64, usize) {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let _path = parts.next()?;
            let bytes = parts.next()?.parse::<u64>().ok()?;
            Some(bytes)
        })
        .fold((0, 0), |(total, count), bytes| (total + bytes, count + 1))
}

fn human_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.0} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

pub(crate) fn cache_runtime_environment(
    workspace: &Path,
    profile: &RuntimeProfile,
    cache_key: &str,
    envs: &[(String, String)],
) {
    let cache_envs = cacheable_runtime_env(envs);
    let _ = write_runtime_env_cache(workspace, profile, cache_key, &cache_envs);
}

pub(crate) fn parse_env_zero(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut envs = Vec::new();
    let entries = bytes.split(|byte| *byte == 0);
    let entries = match bytes
        .split(|byte| *byte == 0)
        .position(|entry| entry == ENV_START_MARKER)
    {
        Some(marker) => entries.skip(marker + 1).collect::<Vec<_>>(),
        None => entries.collect::<Vec<_>>(),
    };

    for entry in entries {
        if entry.is_empty() {
            continue;
        }
        let Some(eq) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let name = String::from_utf8(entry[..eq].to_vec())
            .map_err(|_| "runtime shell environment contains an invalid variable name")?;
        let value = String::from_utf8(entry[eq + 1..].to_vec())
            .map_err(|_| "runtime shell environment contains an invalid variable value")?;
        envs.push((name, value));
    }
    Ok(envs)
}

#[derive(Debug, Eq, PartialEq)]
enum RuntimeEnvCache {
    Hit(Vec<(String, String)>),
    Miss(RuntimeCacheMiss),
}

#[derive(Debug, Eq, PartialEq)]
enum RuntimeCacheMiss {
    Missing,
    FormatChanged,
    StaleInputs,
    InvalidEnvironment,
    MissingStorePaths(Vec<PathBuf>),
}

impl RuntimeCacheMiss {
    fn label(&self) -> &'static str {
        match self {
            Self::Missing => "new",
            Self::FormatChanged => "refresh",
            Self::StaleInputs => "refresh",
            Self::InvalidEnvironment => "refresh",
            Self::MissingStorePaths(_) => "refresh",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Missing => "not yet created".to_string(),
            Self::FormatChanged => "format changed".to_string(),
            Self::StaleInputs => "stale runtime inputs".to_string(),
            Self::InvalidEnvironment => "invalid environment payload".to_string(),
            Self::MissingStorePaths(paths) => {
                format!("missing {} referenced store paths", paths.len())
            }
        }
    }
}

fn read_runtime_env_cache(
    workspace: &Path,
    profile: &RuntimeProfile,
    cache_key: &str,
) -> RuntimeEnvCache {
    let Ok(bytes) = fs::read(runtime_env_cache_path(workspace, profile)) else {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::Missing);
    };
    let Some((magic, rest)) = split_once_byte(&bytes, b'\n') else {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::FormatChanged);
    };
    if magic != RUNTIME_ENV_CACHE_MAGIC.as_bytes() {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::FormatChanged);
    }
    let Some((key, env_bytes)) = split_once_byte(rest, b'\n') else {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::FormatChanged);
    };
    if key != cache_key.as_bytes() {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::StaleInputs);
    }
    let Ok(mut envs) = parse_env_zero(env_bytes) else {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::InvalidEnvironment);
    };
    remove_volatile_runtime_env_values(&mut envs);
    let missing_store_paths = missing_store_roots(&envs);
    if !missing_store_paths.is_empty() {
        return RuntimeEnvCache::Miss(RuntimeCacheMiss::MissingStorePaths(missing_store_paths));
    }
    RuntimeEnvCache::Hit(envs)
}

fn write_runtime_env_cache(
    workspace: &Path,
    profile: &RuntimeProfile,
    cache_key: &str,
    envs: &[(String, String)],
) -> io::Result<()> {
    let state_dir = profile.state_dir(workspace);
    fs::create_dir_all(&state_dir)?;
    let cache_path = runtime_env_cache_path(workspace, profile);
    let tmp_path = state_dir.join(format!(
        "{RUNTIME_ENV_CACHE_FILE}.tmp-{}",
        std::process::id()
    ));
    let mut bytes = Vec::new();
    bytes.extend_from_slice(RUNTIME_ENV_CACHE_MAGIC.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(cache_key.as_bytes());
    bytes.push(b'\n');
    for (name, value) in envs {
        if name.as_bytes().contains(&b'=')
            || name.as_bytes().contains(&0)
            || value.as_bytes().contains(&0)
        {
            continue;
        }
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'=');
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }
    fs::write(&tmp_path, bytes)?;
    fs::rename(tmp_path, cache_path)
}

fn runtime_env_cache_path(workspace: &Path, profile: &RuntimeProfile) -> PathBuf {
    profile.state_dir(workspace).join(RUNTIME_ENV_CACHE_FILE)
}

fn split_once_byte(bytes: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == needle)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

pub(crate) fn inherit_terminal_environment(envs: &mut Vec<(String, String)>) {
    inherit_terminal_environment_from(envs, |name| env::var(name).ok());
}

fn inherit_terminal_environment_from(
    envs: &mut Vec<(String, String)>,
    mut get_env: impl FnMut(&str) -> Option<String>,
) {
    for name in INHERITED_TERMINAL_ENV_VARS {
        envs.retain(|(candidate, _)| candidate != name);
        if let Some(value) = get_env(name).filter(|value| !value.is_empty()) {
            envs.push(((*name).to_string(), value));
        }
    }
}

fn cacheable_runtime_env(envs: &[(String, String)]) -> Vec<(String, String)> {
    envs.iter()
        .filter(|(name, _)| !INHERITED_TERMINAL_ENV_VARS.contains(&name.as_str()))
        .filter(|(name, _)| !TERMINAL_COLOR_ENV_VARS.contains(&name.as_str()))
        .filter(|(name, _)| !VOLATILE_RUNTIME_ENV_VARS.contains(&name.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn remove_volatile_runtime_env_values(envs: &mut Vec<(String, String)>) {
    envs.retain(|(name, _)| {
        !TERMINAL_COLOR_ENV_VARS.contains(&name.as_str())
            && !VOLATILE_RUNTIME_ENV_VARS.contains(&name.as_str())
    });
}

pub(crate) fn filter_nix_output_for_user(output: &std::process::Output) -> std::process::Output {
    let mut filtered = output.clone();
    filtered.stderr = filter_nix_stderr_for_user(&output.stderr);
    filtered
}

fn filter_nix_stderr_for_user(stderr: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(stderr);
    let filtered = text
        .lines()
        .filter(|line| !is_restricted_nix_setting_warning(line))
        .collect::<Vec<_>>()
        .join("\n");
    if filtered.is_empty() {
        Vec::new()
    } else {
        let mut bytes = filtered.into_bytes();
        bytes.push(b'\n');
        bytes
    }
}

fn is_restricted_nix_setting_warning(line: &str) -> bool {
    line.starts_with("warning: ignoring the client-specified setting '")
        && line.ends_with("', because it is a restricted setting and you are not a trusted user")
}

pub(crate) fn missing_store_roots(envs: &[(String, String)]) -> Vec<PathBuf> {
    let mut paths = envs
        .iter()
        .flat_map(|(_, value)| store_roots_in_value(value))
        .filter(|path| !path.exists())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn store_roots_in_value(value: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut rest = value;
    while let Some(index) = rest.find("/nix/store/") {
        let start = &rest[index..];
        let end = start
            .char_indices()
            .find_map(|(offset, character)| (!is_store_path_character(character)).then_some(offset))
            .unwrap_or(start.len());
        let root = &start[..end];
        if root.len() > "/nix/store/".len() {
            roots.push(PathBuf::from(root));
        }
        rest = &start[end..];
    }
    roots.sort();
    roots.dedup();
    roots
}

fn is_store_path_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '/' | '.' | '_' | '+' | '-' | '?' | '=')
}

pub(crate) fn apply_env(command: &mut Command, envs: &[(String, String)]) {
    command.env_clear();
    command.envs(envs.iter().map(|(name, value)| (name, value)));
}

pub(crate) fn add_env_capture_args(command: &mut Command) {
    command.arg("sh").arg("-c").arg(ENV_CAPTURE_SCRIPT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nul_separated_shell_environment() {
        assert_eq!(
            parse_env_zero(b"PATH=/bin\0BAD\0QUOTE=a'b\0").unwrap(),
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("QUOTE".to_string(), "a'b".to_string()),
            ]
        );
    }

    #[test]
    fn nix_command_adds_robo_cache_options() {
        let command = nix_command();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "--option",
                "extra-substituters",
                ROBO_NIX_EXTRA_SUBSTITUTERS,
                "--option",
                "extra-trusted-public-keys",
                ROBO_NIX_EXTRA_TRUSTED_PUBLIC_KEYS,
                "--option",
                "narinfo-cache-negative-ttl",
                "0",
            ]
        );
    }

    #[test]
    fn nix_command_removes_volatile_temp_env() {
        let command = nix_command();
        let removed = command
            .get_envs()
            .filter_map(|(name, value)| {
                value.is_none().then(|| name.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>();

        for name in VOLATILE_RUNTIME_ENV_VARS {
            assert!(removed.contains(&name.to_string()));
        }
    }

    #[test]
    fn git_command_removes_runtime_library_path() {
        let command = git_command(Path::new("/tmp/project"));
        let removed = command
            .get_envs()
            .filter_map(|(name, value)| {
                value.is_none().then(|| name.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>();

        assert!(removed.contains(&"LD_LIBRARY_PATH".to_string()));
    }

    #[test]
    fn parses_runtime_input_output_installables_from_derivation_json() {
        let json = r#"{
          "derivations": {
            "/nix/store/shell.drv": {
              "inputs": {
                "drvs": {
                  "abc-python3-3.11.15.drv": {"outputs": ["out"]},
                  "/nix/store/def-lib.drv": {"outputs": ["dev", "out"]},
                  "ignored-source.drv": {"dynamicOutputs": {}}
                }
              }
            }
          },
          "version": 4
        }"#;

        assert_eq!(
            input_output_installables_from_derivation_json(json).unwrap(),
            vec![
                "/nix/store/abc-python3-3.11.15.drv^out".to_string(),
                "/nix/store/def-lib.drv^dev".to_string(),
                "/nix/store/def-lib.drv^out".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_shell_hook_stdout_before_marker() {
        assert_eq!(
            parse_env_zero(b"hello from shell hook\n\0robo-nix-env-start\0PATH=/bin\0").unwrap(),
            vec![("PATH".to_string(), "/bin".to_string())]
        );
    }

    #[test]
    fn runtime_env_cache_round_trips_nul_environment() {
        let root = temp_project("runtime-env-cache");
        let profile = RuntimeProfile::default();
        fs::create_dir_all(profile.state_dir(&root)).unwrap();
        let envs = vec![
            ("PATH".to_string(), "/bin".to_string()),
            (
                "ROBO_NIX_COMPONENTS".to_string(),
                "native-build".to_string(),
            ),
        ];

        write_runtime_env_cache(&root, &profile, "cache-key", &envs).unwrap();

        assert_eq!(
            read_runtime_env_cache(&root, &profile, "cache-key"),
            RuntimeEnvCache::Hit(envs)
        );
        assert_eq!(
            read_runtime_env_cache(&root, &profile, "other-key"),
            RuntimeEnvCache::Miss(RuntimeCacheMiss::StaleInputs)
        );

        cleanup(root);
    }

    #[test]
    fn runtime_env_cache_reports_miss_reasons() {
        let root = temp_project("runtime-env-cache-reasons");
        let profile = RuntimeProfile::default();

        assert_eq!(
            read_runtime_env_cache(&root, &profile, "cache-key"),
            RuntimeEnvCache::Miss(RuntimeCacheMiss::Missing)
        );

        fs::create_dir_all(profile.state_dir(&root)).unwrap();
        fs::write(runtime_env_cache_path(&root, &profile), "bad-cache").unwrap();

        assert_eq!(
            read_runtime_env_cache(&root, &profile, "cache-key"),
            RuntimeEnvCache::Miss(RuntimeCacheMiss::FormatChanged)
        );

        cleanup(root);
    }

    #[test]
    fn runtime_cache_progress_labels_are_user_facing() {
        assert_eq!(RuntimeCacheMiss::Missing.label(), "new");
        assert_eq!(RuntimeCacheMiss::StaleInputs.label(), "refresh");
        assert_eq!(RuntimeCacheMiss::FormatChanged.label(), "refresh");
        assert_eq!(RuntimeCacheMiss::InvalidEnvironment.label(), "refresh");
        assert_eq!(
            RuntimeCacheMiss::MissingStorePaths(vec![PathBuf::from("/nix/store/example")]).label(),
            "refresh"
        );
    }

    #[test]
    fn terminal_identity_overrides_captured_runtime_environment() {
        let mut envs = vec![
            ("PATH".to_string(), "/bin".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
        ];

        inherit_terminal_environment_from(&mut envs, |name| match name {
            "TERM" => Some("tmux-256color".to_string()),
            "COLORTERM" => Some("truecolor".to_string()),
            _ => None,
        });

        assert_eq!(
            shell_env_value(&envs, "TERM").map(String::as_str),
            Some("tmux-256color")
        );
        assert_eq!(
            shell_env_value(&envs, "COLORTERM").map(String::as_str),
            Some("truecolor")
        );
    }

    #[test]
    fn runtime_cache_excludes_terminal_identity() {
        let envs = vec![
            ("PATH".to_string(), "/bin".to_string()),
            ("TERM".to_string(), "tmux-256color".to_string()),
            ("TMUX".to_string(), "/tmp/tmux-1000/default,1,0".to_string()),
            ("NO_COLOR".to_string(), "1".to_string()),
            ("FORCE_COLOR".to_string(), "1".to_string()),
            ("TMPDIR".to_string(), "/tmp/nix-shell.deleted".to_string()),
            ("PWD".to_string(), "/workspace".to_string()),
        ];

        let cache_envs = cacheable_runtime_env(&envs);

        assert_eq!(
            shell_env_value(&cache_envs, "PATH").map(String::as_str),
            Some("/bin")
        );
        assert!(shell_env_value(&cache_envs, "TERM").is_none());
        assert!(shell_env_value(&cache_envs, "TMUX").is_none());
        assert!(shell_env_value(&cache_envs, "NO_COLOR").is_none());
        assert!(shell_env_value(&cache_envs, "FORCE_COLOR").is_none());
        assert!(shell_env_value(&cache_envs, "TMPDIR").is_none());
        assert!(shell_env_value(&cache_envs, "PWD").is_none());
    }

    #[test]
    fn captured_runtime_env_excludes_volatile_shell_values() {
        let mut envs = vec![
            ("PATH".to_string(), "/bin".to_string()),
            ("NO_COLOR".to_string(), "1".to_string()),
            ("CLICOLOR_FORCE".to_string(), "1".to_string()),
            ("TMPDIR".to_string(), "/tmp/nix-shell.deleted".to_string()),
            ("TEMP".to_string(), "/tmp/nix-shell.deleted".to_string()),
            ("PWD".to_string(), "/workspace".to_string()),
            ("OLDPWD".to_string(), "/workspace/old".to_string()),
        ];

        remove_volatile_runtime_env_values(&mut envs);

        assert_eq!(envs, vec![("PATH".to_string(), "/bin".to_string())]);
    }

    #[test]
    fn user_nix_stderr_filters_restricted_setting_warnings() {
        let stderr = b"warning: ignoring the client-specified setting 'trusted-public-keys', because it is a restricted setting and you are not a trusted user\nbuilding '/nix/store/example.drv'...\nerror: failed\n";

        assert_eq!(
            filter_nix_stderr_for_user(stderr),
            b"building '/nix/store/example.drv'...\nerror: failed\n"
        );
    }

    #[test]
    fn store_paths_are_extracted_from_env_values() {
        assert_eq!(
            store_roots_in_value("/nix/store/abc-package/lib:/other"),
            vec![PathBuf::from("/nix/store/abc-package/lib")]
        );
    }

    #[test]
    fn apply_env_clears_values_absent_from_captured_environment() {
        let path = std::env::var("PATH").unwrap_or_else(|_| "/bin:/usr/bin".to_string());
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("printf '%s:%s' \"${ROBO_NIX_TEST_LEAK-unset}\" \"${KEEP_ME-unset}\"")
            .env("ROBO_NIX_TEST_LEAK", "leak");

        apply_env(
            &mut command,
            &[
                ("PATH".to_string(), path),
                ("KEEP_ME".to_string(), "1".to_string()),
            ],
        );

        let output = command.output().expect("test shell should run");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset:1");
    }

    #[test]
    fn untracked_git_runtime_inputs_use_path_flake_ref() {
        assert_eq!(
            workspace_flake_ref_for_git_state(Path::new("/tmp/project"), true),
            "path:/tmp/project"
        );
        assert_eq!(
            workspace_flake_ref_for_git_state(Path::new("/tmp/project"), false),
            "."
        );
    }

    #[test]
    fn detects_untracked_git_runtime_inputs() {
        let root = temp_project("untracked-runtime-inputs");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.12\n").unwrap();
        fs::write(root.join("flake.nix"), "{ outputs = _: {}; }\n").unwrap();
        fs::write(root.join("robo.nix"), "{}\n").unwrap();

        let init = Command::new("git").current_dir(&root).arg("init").output();
        let Ok(init) = init else {
            cleanup(root);
            return;
        };
        if !init.status.success() {
            cleanup(root);
            return;
        }

        let add = Command::new("git")
            .current_dir(&root)
            .arg("add")
            .arg(".python-version")
            .arg("flake.nix")
            .output()
            .unwrap();
        assert!(add.status.success());
        assert!(has_untracked_git_runtime_inputs(&root));

        let add = Command::new("git")
            .current_dir(&root)
            .arg("add")
            .arg("robo.nix")
            .output()
            .unwrap();
        assert!(add.status.success());
        assert!(!has_untracked_git_runtime_inputs(&root));

        cleanup(root);
    }

    #[test]
    fn parses_path_info_size_output() {
        assert_eq!(
            parse_path_info_size_output(
                "/nix/store/aaa-one        1024\n/nix/store/bbb-two\t2048\nbad\n"
            ),
            (3072, 2)
        );
    }

    #[test]
    fn formats_runtime_disk_estimate_status() {
        let estimate = RuntimeDiskEstimate {
            known_bytes: 3 * 1024 * 1024 * 1024 + 512 * 1024 * 1024,
            known_paths: 42,
            unknown_paths: 2,
        };

        assert_eq!(
            estimate.status_line("shell"),
            "approximate runtime closure 3.5 GiB across 42 store paths; 2 paths not yet sized"
        );
    }

    fn shell_env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a String> {
        envs.iter()
            .find_map(|(candidate, value)| (candidate == name).then_some(value))
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-host-cuda-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
