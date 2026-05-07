use std::cell::RefCell;
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::{Config, LabelKind, error, hint, human_duration, inline, label, status};

use super::super::nix::{
    add_runtime_source_override, command_for_runtime, exit_code, hint_native_cuda_link_failure,
};
use super::{RuntimeState, host_bridge, nix_system_name};

const SHELL_ENV_CAPTURE_SCRIPT: &str = "source /dev/stdin >/dev/null; \
     if [ -n \"${shellHook:-}\" ]; then eval \"$shellHook\" >/dev/null; fi; \
     env -0";

pub(super) fn load_shell_env_script(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<u8>, ExitCode> {
    let mut command = command_for_runtime(config);
    command.arg("print-dev-env");
    add_runtime_source_override(&mut command);
    command.arg(".#default");
    if let Some(progress) = progress {
        progress.set("shell: evaluating and realizing dev shell");
    }
    if progress.is_none() {
        status(config, "shell: evaluating and realizing dev shell");
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => {
            error(
                config,
                &format!("failed to load shell environment: {err}"),
            );
            return Err(ExitCode::from(1));
        }
    };

    if !output.status.success() {
        error(config, "shell environment failed to load");
        print_captured("stdout", &output.stdout);
        print_captured("stderr", &output.stderr);
        hint_native_cuda_link_failure(config, &output);
        return Err(exit_code(output.status.code()));
    }

    Ok(output.stdout)
}

pub(super) fn materialize_shell_env(
    script: &[u8],
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    if let Some(progress) = progress {
        progress.set("shell: capturing shell environment");
    }
    let mut env = match shell_env_exports(script) {
        Ok(env) => env,
        Err(message) => {
            error(config, &message);
            return Err(ExitCode::from(1));
        }
    };
    if let Some(progress) = progress {
        progress.set("shell: applying runtime exports");
    }
    host_bridge::append_host_cuda_driver_bridge(&mut env);
    host_bridge::append_host_graphics_bridge(&mut env);
    append_shell_state(&mut env);
    Ok(env)
}

pub(super) fn load_cached_or_refresh_shell_env(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    match read_shell_env_cache() {
        Ok(Some(env)) => {
            if let Some(progress) = progress {
                progress.set("shell: using cached runtime shell");
            } else {
                status(config, "shell: using cached runtime shell");
            }
            Ok(env)
        }
        Ok(None) => refresh_shell_env_cache(config, progress),
        Err(message) => {
            if config.debug {
                hint(config, &format!("ignoring stale shell cache: {message}"));
            }
            refresh_shell_env_cache(config, progress)
        }
    }
}

pub(super) fn write_shell_env_cache_if_possible(env: &[(String, String)], config: Config) {
    if let Err(message) = write_shell_env_cache(env) {
        if config.debug {
            hint(config, &format!("failed to write shell cache: {message}"));
        }
    }
}

pub(super) struct ShellProgress {
    config: Config,
    tree: Option<ShellProgressTree>,
    started_at: Instant,
    active_step: RefCell<Option<ActiveStep>>,
}

struct ActiveStep {
    message: String,
    started_at: Instant,
}

struct ShellProgressTree {
    multi: MultiProgress,
    root: ProgressBar,
    active_child: RefCell<Option<ProgressBar>>,
}

impl ShellProgress {
    pub(super) fn new(config: Config, message: &str) -> Self {
        let started_at = Instant::now();
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self {
                config,
                tree: None,
                started_at,
                active_step: RefCell::new(None),
            };
        }

        let tree = ShellProgressTree::new(config, message);
        Self {
            config,
            tree: Some(tree),
            started_at,
            active_step: RefCell::new(Some(ActiveStep {
                message: message.to_string(),
                started_at,
            })),
        }
    }

    pub(super) fn set(&self, message: &str) {
        if let Some(tree) = &self.tree {
            self.finish_active_step("├");
            *self.active_step.borrow_mut() = Some(ActiveStep {
                message: message.to_string(),
                started_at: Instant::now(),
            });
            tree.start_child(self.config, message);
        } else {
            status(self.config, message);
        }
    }

    pub(super) fn finish(&mut self) {
        if self.tree.is_some() {
            self.finish_active_step("└");
        }
        if let Some(tree) = &self.tree {
            tree.finish_clear();
        }
    }

    pub(super) fn finish_ready(&mut self) {
        if self.tree.is_some() {
            self.finish_active_step("└");
        }
        if let Some(tree) = &self.tree {
            tree.finish_ready(self.config, self.started_at.elapsed());
        } else {
            status(self.config, "robo ready");
        }
    }

    fn finish_active_step(&self, branch: &str) {
        let Some(step) = self.active_step.borrow_mut().take() else {
            return;
        };
        if let Some(tree) = &self.tree {
            tree.finish_child(self.config, branch, &step.message, step.started_at.elapsed());
        }
    }
}

impl ShellProgressTree {
    fn new(config: Config, message: &str) -> Self {
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
        let root = multi.add(tree_root_bar(config, "robo shell"));
        let child = multi.add(tree_child_bar(config, message));
        Self {
            multi,
            root,
            active_child: RefCell::new(Some(child)),
        }
    }

    fn start_child(&self, config: Config, message: &str) {
        *self.active_child.borrow_mut() = Some(self.multi.add(tree_child_bar(config, message)));
    }

    fn finish_child(&self, config: Config, branch: &str, message: &str, duration: Duration) {
        let line = format!(
            "  {} {} {} {}",
            label(config, branch, LabelKind::Hint),
            label(config, "✓", LabelKind::Ok),
            tree_message(config, message),
            label(config, &human_duration(duration), LabelKind::Hint)
        );
        if let Some(child) = self.active_child.borrow_mut().take() {
            child.set_style(tree_finished_style());
            child.finish_with_message(line);
        } else {
            eprintln!("{line}");
        }
    }

    fn finish_ready(&self, config: Config, duration: Duration) {
        self.root.set_style(tree_finished_style());
        self.root.finish_with_message(format!(
            "{} {} {}",
            label(config, "✓", LabelKind::Ok),
            inline(config, "robo ready"),
            label(config, &human_duration(duration), LabelKind::Ok)
        ));
    }

    fn finish_clear(&self) {
        if let Some(child) = self.active_child.borrow_mut().take() {
            child.finish_and_clear();
        }
        self.root.finish_and_clear();
    }
}

fn tree_root_bar(config: Config, message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg} {elapsed_precise:.dim}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    bar.set_message(inline(config, message));
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

fn tree_child_bar(config: Config, message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("  └ {spinner:.cyan} {msg} {elapsed_precise:.dim}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    bar.set_message(tree_message(config, message));
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

fn tree_finished_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}").unwrap_or_else(|_| ProgressStyle::default_bar())
}

fn tree_message(config: Config, message: &str) -> String {
    let Some((phase, rest)) = message.split_once(": ") else {
        return inline(config, message);
    };

    format!(
        "{} {}",
        label(config, &format!("{phase}:"), LabelKind::Status),
        inline(config, rest)
    )
}

pub(super) fn shell_env_value<'a>(envs: &'a [(String, String)], name: &str) -> Option<&'a String> {
    envs.iter()
        .rev()
        .find_map(|(candidate, value)| (candidate == name).then_some(value))
}

pub(super) fn set_shell_env(envs: &mut Vec<(String, String)>, name: &str, value: String) {
    if let Some((_, existing)) = envs.iter_mut().find(|(candidate, _)| candidate == name) {
        *existing = value;
    } else {
        envs.push((name.to_string(), value));
    }
}

fn load_shell_env(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    let script = load_shell_env_script(config, progress)?;
    materialize_shell_env(&script, config, progress)
}

fn refresh_shell_env_cache(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    let env = load_shell_env(config, progress)?;
    write_shell_env_cache_if_possible(&env, config);
    Ok(env)
}

fn print_captured(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    eprintln!("--- shell {label} ---");
    eprint!("{}", String::from_utf8_lossy(bytes));
}

fn append_shell_state(envs: &mut Vec<(String, String)>) {
    let state = RuntimeState::read();
    set_shell_env(envs, "ROBO_NIX_ACTIVE", "1".to_string());
    set_shell_env(envs, "ROBO_NIX_ENV_NAME", state.env_name.clone());
    set_shell_env(
        envs,
        "ROBO_NIX_PYTHON_VERSION",
        state.python_version.clone(),
    );
    set_shell_env(envs, "WORKSPACE_ROOT", state.workspace.clone());
    set_shell_env(
        envs,
        "ROBO_NIX_PROMPT_PREFIX",
        "[robo]".to_string(),
    );

    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let parent = parent.display().to_string();
            let base = shell_env_value(envs, "PATH")
                .cloned()
                .or_else(|| env::var("PATH").ok())
                .unwrap_or_default();
            set_shell_env(envs, "PATH", format!("{parent}:{base}"));
        }
    }
}

fn shell_env_cache_dir() -> PathBuf {
    PathBuf::from(".robo-nix")
}

fn shell_env_cache_path() -> PathBuf {
    shell_env_cache_dir().join("shell-env")
}

fn shell_env_cache_key_path() -> PathBuf {
    shell_env_cache_dir().join("shell-env.key")
}

fn read_shell_env_cache() -> Result<Option<Vec<(String, String)>>, String> {
    let key_path = shell_env_cache_key_path();
    let env_path = shell_env_cache_path();
    if !key_path.exists() || !env_path.exists() {
        return Ok(None);
    }

    let expected = shell_env_cache_key();
    let actual = fs::read_to_string(&key_path)
        .map_err(|err| format!("failed to read {}: {err}", key_path.display()))?;
    if actual.trim() != expected {
        return Ok(None);
    }

    let bytes = fs::read(&env_path)
        .map_err(|err| format!("failed to read {}: {err}", env_path.display()))?;
    parse_cached_shell_env(&bytes).map(Some)
}

fn write_shell_env_cache(env: &[(String, String)]) -> Result<(), String> {
    let dir = shell_env_cache_dir();
    fs::create_dir_all(&dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    fs::write(shell_env_cache_path(), serialize_shell_env(env))
        .map_err(|err| format!("failed to write shell env cache: {err}"))?;
    fs::write(shell_env_cache_key_path(), format!("{}\n", shell_env_cache_key()))
        .map_err(|err| format!("failed to write shell env cache key: {err}"))?;
    Ok(())
}

fn serialize_shell_env(env: &[(String, String)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, value) in env {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn parse_cached_shell_env(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let chunks: Vec<_> = bytes.split(|byte| *byte == 0).collect();
    let mut entries = chunks.as_slice();
    if entries.last().is_some_and(|entry| entry.is_empty()) {
        entries = &entries[..entries.len() - 1];
    }
    if entries.len() % 2 != 0 {
        return Err("shell env cache is truncated".to_string());
    }

    entries
        .chunks(2)
        .map(|pair| {
            let name = String::from_utf8(pair[0].to_vec())
                .map_err(|_| "shell env cache contains an invalid variable name".to_string())?;
            let value = String::from_utf8(pair[1].to_vec())
                .map_err(|_| "shell env cache contains an invalid variable value".to_string())?;
            Ok((name, value))
        })
        .collect()
}

fn shell_env_cache_key() -> String {
    let mut hasher = DefaultHasher::new();
    nix_system_name().hash(&mut hasher);
    env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .hash(&mut hasher);
    env::var("ROBO_NIX_DEFAULT_SOURCE_URL").ok().hash(&mut hasher);
    env::var("ROBO_NIX_RUNTIME_SOURCE_URL").ok().hash(&mut hasher);
    env::var("ROBO_NIX_DISABLE_HOST_CUDA_AUTO")
        .ok()
        .hash(&mut hasher);
    env::var("ROBO_NIX_DISABLE_HOST_GRAPHICS_AUTO")
        .ok()
        .hash(&mut hasher);
    crate::runtime::find_host_libcuda().hash(&mut hasher);
    crate::runtime::find_host_nvidia_egl_vendor_file().hash(&mut hasher);
    crate::runtime::find_host_nvidia_vulkan_icd_file().hash(&mut hasher);
    for name in [
        "ROBO_NIX_WORKSPACE",
        "ROBO_NIX_LIBCUDA_PATH",
        "ROBO_NIX_CUDA_ROOT",
        "UV_PROJECT_ENVIRONMENT",
    ] {
        name.hash(&mut hasher);
        env::var(name).ok().hash(&mut hasher);
    }

    for path in [
        "flake.nix",
        "flake.lock",
        "robo.nix",
        ".python-version",
        "pyproject.toml",
        "uv.lock",
    ] {
        path.hash(&mut hasher);
        match fs::read(path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => 0_u8.hash(&mut hasher),
        }
    }
    hash_venv_cmake_prefixes(&mut hasher);

    format!("{:016x}", hasher.finish())
}

fn hash_venv_cmake_prefixes<H: Hasher>(hasher: &mut H) {
    let pyvenv = Path::new(".venv/pyvenv.cfg");
    pyvenv.hash(hasher);
    match fs::read(pyvenv) {
        Ok(bytes) => bytes.hash(hasher),
        Err(_) => 0_u8.hash(hasher),
    }

    let mut prefixes = Vec::new();
    let Ok(python_dirs) = fs::read_dir(".venv/lib") else {
        prefixes.hash(hasher);
        return;
    };
    for python_dir in python_dirs.flatten() {
        let site_packages = python_dir.path().join("site-packages");
        if !site_packages.is_dir() {
            continue;
        }
        if site_packages.join("share/cmake").is_dir() {
            prefixes.push(site_packages.display().to_string());
        }
        let Ok(entries) = fs::read_dir(&site_packages) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("share/cmake").is_dir() {
                prefixes.push(path.display().to_string());
            }
        }
    }
    prefixes.sort();
    prefixes.hash(hasher);
}

fn shell_env_exports(script: &[u8]) -> Result<Vec<(String, String)>, String> {
    let baseline: HashMap<_, _> = env::vars().collect();
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(SHELL_ENV_CAPTURE_SCRIPT)
        .env("ROBO_NIX_QUIET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to materialize shell environment: {err}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "failed to open shell environment stdin".to_string())?
        .write_all(script)
        .map_err(|err| format!("failed to write shell environment: {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to read shell environment: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("shell setup failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(name, value)| should_export_shell_var(name, value, &baseline))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect())
}

fn should_export_shell_var(name: &str, value: &str, baseline: &HashMap<String, String>) -> bool {
    !is_shell_export_blocked(name)
        && is_shell_identifier(name)
        && baseline.get(name).is_none_or(|baseline| baseline != value)
}

fn is_shell_export_blocked(name: &str) -> bool {
    matches!(
        name,
        "" | "_" | "PWD" | "OLDPWD" | "SHLVL" | "SHELL" | "shellHook" | "ROBO_NIX_QUIET"
    ) || name.starts_with("BASH")
}

fn is_shell_identifier(name: &str) -> bool {
    name.chars().enumerate().all(|(index, ch)| {
        ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_env_cache_round_trips_values() {
        let env = vec![
            ("PATH".to_string(), "/nix/store/bin:/usr/bin".to_string()),
            ("ROBO_NIX_ACTIVE".to_string(), "1".to_string()),
            ("CMAKE_PREFIX_PATH".to_string(), "/tmp/pkg/share/cmake".to_string()),
        ];

        assert_eq!(
            parse_cached_shell_env(&serialize_shell_env(&env)).expect("cache should parse"),
            env
        );
    }

    #[test]
    fn shell_env_cache_rejects_truncated_entries() {
        let bytes = b"PATH\0/bin\0ROBO_NIX_ACTIVE";

        assert_eq!(
            parse_cached_shell_env(bytes).expect_err("truncated cache should fail"),
            "shell env cache is truncated"
        );
    }

    #[test]
    fn shell_env_exports_skip_unchanged_parent_values() {
        let baseline = HashMap::from([
            ("EXPECTED_SHELL".to_string(), "/tmp/first".to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
        ]);

        assert!(!should_export_shell_var(
            "EXPECTED_SHELL",
            "/tmp/first",
            &baseline
        ));
        assert!(should_export_shell_var("PATH", "/nix/bin:/usr/bin", &baseline));
        assert!(!should_export_shell_var("SHELL", "/nix/store/bash", &baseline));
    }
}
