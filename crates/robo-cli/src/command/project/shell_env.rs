use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::{Config, HiddenCursor, LabelKind, error, hint, human_duration, inline, label, status};

use super::super::nix::{
    add_runtime_source_override, command_for_runtime, command_for_runtime_progress, exit_code,
    hint_native_cuda_link_failure,
};
use super::{RuntimeState, host_bridge, nix_system_name};

const SHELL_ENV_CAPTURE_SCRIPT: &str = "source /dev/stdin >/dev/null; \
     if [ -n \"${shellHook:-}\" ]; then eval \"$shellHook\" >/dev/null; fi; \
     env -0";
const SHELL_PROGRESS_SPINNER_FRAMES: &[&str] =
    &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SHELL_STEP_WIDTH: usize = 34;
const SHELL_META_WIDTH: usize = 12;
const SHELL_DURATION_WIDTH: usize = 6;
const SHELL_READY_WIDTH: usize = 51;

pub(super) fn load_shell_env_script(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<u8>, ExitCode> {
    let mut command = if progress.is_some() {
        command_for_runtime_progress(config)
    } else {
        command_for_runtime(config)
    };
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
    let output = match run_shell_env_command(command, progress) {
        Ok(output) => output,
        Err(err) => {
            error(
                config,
                &format!("failed to load shell environment: {}", err),
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

fn run_shell_env_command(
    mut command: Command,
    progress: Option<&ShellProgress>,
) -> Result<std::process::Output, String> {
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to start command: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture shell environment stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture shell environment stderr".to_string())?;

    let stdout_reader = thread::spawn(move || {
        let mut stdout = stdout;
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let detail_sink = progress.and_then(ShellProgress::detail_sink);
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = BufReader::new(stderr);
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = reader.read_until(b'\n', &mut line)?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&line);
            if let Some(progress) = &detail_sink {
                progress.detail(&line);
            }
        }
        Ok::<Vec<u8>, std::io::Error>(bytes)
    });

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for shell environment command: {err}"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "failed to join shell environment stdout reader".to_string())?
        .map_err(|err| format!("failed to read shell environment stdout: {err}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "failed to join shell environment stderr reader".to_string())?
        .map_err(|err| format!("failed to read shell environment stderr: {err}"))?;

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn shell_env_progress_detail(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.rsplit('\r').next().unwrap_or(&text);
    let detail = text
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    if detail.is_empty() {
        return None;
    }
    if detail.starts_with("linking ") || detail.starts_with("evaluating file '") {
        return None;
    }
    Some(truncate_progress_detail(&detail))
}

fn nix_evaluated_package_path(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.rsplit('\r').next().unwrap_or(&text);
    let path = text
        .trim()
        .strip_prefix("evaluating file '")?
        .strip_suffix("'")?;
    path.ends_with("/package.nix").then(|| path.to_string())
}

fn truncate_progress_detail(detail: &str) -> String {
    const MAX_DETAIL_CHARS: usize = 110;
    if detail.chars().count() <= MAX_DETAIL_CHARS {
        return detail.to_string();
    }
    let mut truncated = detail.chars().take(MAX_DETAIL_CHARS - 1).collect::<String>();
    truncated.push('…');
    truncated
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
                progress.finish_cached_active();
            } else {
                status(config, "shell: evaluating and realizing dev shell cached");
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

pub(super) fn refresh_shell_env_for_hook(
    config: Config,
    progress: Option<&ShellProgress>,
) -> Result<Vec<(String, String)>, ExitCode> {
    let script = load_shell_env_script(config, progress)?;
    let env = materialize_shell_env(&script, config, progress)?;
    write_shell_env_cache_if_possible(&env, config);
    Ok(env)
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
    bar: ProgressBar,
    state: Arc<Mutex<ShellProgressTreeState>>,
    stop_ticker: Arc<AtomicBool>,
    ticker: RefCell<Option<JoinHandle<()>>>,
    _cursor: HiddenCursor,
}

struct ShellProgressTreeState {
    completed: Vec<String>,
    active_message: String,
    active_started_at: Instant,
    evaluated_packages: HashSet<String>,
    details: VecDeque<String>,
}

struct ShellProgressDetailSink {
    config: Config,
    bar: ProgressBar,
    state: Arc<Mutex<ShellProgressTreeState>>,
}

impl ShellProgressDetailSink {
    fn detail(&self, bytes: &[u8]) {
        let mut state = self.state.lock().expect("shell progress state poisoned");
        if let Some(package) = nix_evaluated_package_path(bytes) {
            state.evaluated_packages.insert(package);
        }
        if let Some(detail) = shell_env_progress_detail(bytes) {
            if state.details.back() != Some(&detail) {
                state.details.push_back(detail);
                while state.details.len() > 4 {
                    state.details.pop_front();
                }
            }
        }
        self.bar.set_message(render_live_tree(self.config, &state));
    }
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
        if self.active_step.borrow().as_ref().map(|step| step.message.as_str()) == Some(message) {
            return;
        }
        if let Some(tree) = &self.tree {
            self.finish_active_step();
            *self.active_step.borrow_mut() = Some(ActiveStep {
                message: message.to_string(),
                started_at: Instant::now(),
            });
            tree.start_child(self.config, message);
        } else {
            status(self.config, message);
        }
    }

    pub(super) fn finish_cached_active(&self) {
        self.finish_active_step_with_suffix(Some("cached"));
    }

    pub(super) fn finish(&mut self) {
        if self.tree.is_some() {
            self.finish_active_step();
        }
        if let Some(tree) = &self.tree {
            tree.finish_clear();
        }
    }

    pub(super) fn finish_ready(&mut self) {
        if self.tree.is_some() {
            self.finish_active_step();
        }
        if let Some(tree) = &self.tree {
            tree.finish_ready(self.config, self.started_at.elapsed());
        } else {
            status(self.config, "robo ready");
        }
    }

    fn detail_sink(&self) -> Option<ShellProgressDetailSink> {
        self.tree.as_ref().map(|tree| ShellProgressDetailSink {
            config: self.config,
            bar: tree.bar.clone(),
            state: Arc::clone(&tree.state),
        })
    }

    fn finish_active_step(&self) {
        self.finish_active_step_with_suffix(None);
    }

    fn finish_active_step_with_suffix(&self, suffix: Option<&str>) {
        let Some(step) = self.active_step.borrow_mut().take() else {
            return;
        };
        if let Some(tree) = &self.tree {
            tree.finish_child(
                self.config,
                &step.message,
                suffix,
                step.started_at.elapsed(),
            );
        }
    }
}

impl ShellProgressTree {
    fn new(config: Config, message: &str) -> Self {
        let bar = tree_bar(config);
        let state = Arc::new(Mutex::new(ShellProgressTreeState {
            completed: Vec::new(),
            active_message: message.to_string(),
            active_started_at: Instant::now(),
            evaluated_packages: HashSet::new(),
            details: VecDeque::new(),
        }));
        let stop_ticker = Arc::new(AtomicBool::new(false));
        let ticker = spawn_tree_ticker(config, &bar, &state, &stop_ticker);
        let tree = Self {
            bar,
            state,
            stop_ticker,
            ticker: RefCell::new(Some(ticker)),
            _cursor: HiddenCursor::new(),
        };
        tree.render_live(config);
        tree
    }

    fn start_child(&self, config: Config, message: &str) {
        let mut state = self.state.lock().expect("shell progress state poisoned");
        state.active_message = message.to_string();
        state.active_started_at = Instant::now();
        state.evaluated_packages.clear();
        state.details.clear();
        drop(state);
        self.render_live(config);
    }

    fn finish_child(
        &self,
        config: Config,
        message: &str,
        suffix: Option<&str>,
        duration: Duration,
    ) {
        let mut state = self.state.lock().expect("shell progress state poisoned");
        let evaluated_packages = state.evaluated_packages.len();
        state.completed.push(completed_tree_line(
            config,
            message,
            suffix,
            evaluated_packages,
            duration,
        ));
        state.evaluated_packages.clear();
    }

    fn finish_ready(&self, config: Config, duration: Duration) {
        self.stop_ticker();
        self.bar.set_style(tree_finished_style());
        let state = self.state.lock().expect("shell progress state poisoned");
        self.bar
            .finish_with_message(render_finished_tree(config, &state.completed, duration));
        self._cursor.show();
    }

    fn finish_clear(&self) {
        self.stop_ticker();
        self.bar.finish_and_clear();
        self._cursor.show();
    }

    fn render_live(&self, config: Config) {
        let state = self.state.lock().expect("shell progress state poisoned");
        self.bar.set_message(render_live_tree(config, &state));
    }

    fn stop_ticker(&self) {
        self.stop_ticker.store(true, Ordering::Relaxed);
        if let Some(ticker) = self.ticker.borrow_mut().take() {
            let _ = ticker.join();
        }
    }
}

impl Drop for ShellProgressTree {
    fn drop(&mut self) {
        self.stop_ticker();
    }
}

fn spawn_tree_ticker(
    config: Config,
    bar: &ProgressBar,
    state: &Arc<Mutex<ShellProgressTreeState>>,
    stop_ticker: &Arc<AtomicBool>,
) -> JoinHandle<()> {
    let bar = bar.clone();
    let state = Arc::clone(state);
    let stop_ticker = Arc::clone(stop_ticker);
    thread::spawn(move || {
        while !stop_ticker.load(Ordering::Relaxed) {
            if let Ok(state) = state.lock() {
                bar.set_message(render_live_tree(config, &state));
            }
            thread::sleep(Duration::from_millis(80));
        }
    })
}

fn tree_bar(config: Config) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_draw_target(ProgressDrawTarget::stderr());
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(SHELL_PROGRESS_SPINNER_FRAMES),
    );
    bar.enable_steady_tick(Duration::from_millis(80));
    bar.set_message(shell_tree_heading(config));
    bar
}

fn render_live_tree(config: Config, state: &ShellProgressTreeState) -> String {
    let mut lines = vec![shell_tree_heading(config)];
    lines.extend(state.completed.iter().cloned());
    if !state.active_message.is_empty() {
        let active_elapsed = state.active_started_at.elapsed();
        let line = format!(
            "  {} {} {}",
            label(config, "└", LabelKind::Hint),
            label(
                config,
                active_spinner_for_elapsed(active_elapsed),
                LabelKind::Hint
            ),
            progress_tree_fields(
                config,
                &state.active_message,
                state.evaluated_packages.len(),
                active_elapsed,
            )
        );
        lines.push(line);
    }
    for detail in &state.details {
        lines.push(format!("    {}", label(config, detail, LabelKind::Hint)));
    }
    lines.join("\n")
}

fn active_spinner_for_elapsed(elapsed: Duration) -> &'static str {
    let frame = (elapsed.as_millis() / 80) as usize % SHELL_PROGRESS_SPINNER_FRAMES.len();
    SHELL_PROGRESS_SPINNER_FRAMES[frame]
}

fn render_finished_tree(config: Config, completed: &[String], duration: Duration) -> String {
    let mut lines = vec![format_ready_line(config, duration)];
    lines.extend(
        completed
            .iter()
            .filter(|line| !line.is_empty())
            .cloned(),
    );
    lines.join("\n")
}

fn format_ready_line(config: Config, duration: Duration) -> String {
    format!(
        "{} {} {}",
        label(config, "✓", LabelKind::Ok),
        pad_display(label(config, "robo ready", LabelKind::Ok), SHELL_READY_WIDTH),
        label(
            config,
            &format!("{:>SHELL_DURATION_WIDTH$}", human_duration(duration)),
            LabelKind::Ok,
        )
    )
}

fn completed_tree_line(
    config: Config,
    message: &str,
    suffix: Option<&str>,
    evaluated_packages: usize,
    duration: Duration,
) -> String {
    if message == "shell: launching shell" {
        return String::new();
    }

    format!(
        "  {} {} {}",
        label(config, "└", LabelKind::Hint),
        label(config, "✓", LabelKind::SecondaryOk),
        completed_tree_fields(config, message, suffix, evaluated_packages, duration)
    )
}

fn completed_tree_fields(
    config: Config,
    message: &str,
    suffix: Option<&str>,
    evaluated_packages: usize,
    duration: Duration,
) -> String {
    tree_fields(
        config,
        message,
        tree_metadata(message, suffix, evaluated_packages),
        duration,
    )
}

fn progress_tree_fields(
    config: Config,
    message: &str,
    evaluated_packages: usize,
    duration: Duration,
) -> String {
    tree_fields(
        config,
        message,
        tree_metadata(message, None, evaluated_packages),
        duration,
    )
}

fn tree_fields(
    config: Config,
    message: &str,
    metadata: Option<TreeMetadata>,
    duration: Duration,
) -> String {
    let metadata = metadata
        .map(|metadata| label(config, &metadata.text, metadata.kind))
        .unwrap_or_default();
    format!(
        "{} {} {}",
        pad_display(inline(config, display_shell_step(message)), SHELL_STEP_WIDTH),
        pad_display(metadata, SHELL_META_WIDTH),
        label(
            config,
            &format!("{:>SHELL_DURATION_WIDTH$}", human_duration(duration)),
            LabelKind::Hint,
        )
    )
}

struct TreeMetadata {
    text: String,
    kind: LabelKind,
}

fn tree_metadata(
    message: &str,
    suffix: Option<&str>,
    evaluated_packages: usize,
) -> Option<TreeMetadata> {
    if evaluated_packages > 0 && message == "shell: evaluating and realizing dev shell" {
        return Some(TreeMetadata {
            text: format!("{evaluated_packages} packages"),
            kind: LabelKind::Status,
        });
    }
    suffix.map(|suffix| TreeMetadata {
        text: suffix.to_string(),
        kind: match suffix {
            "skipped" => LabelKind::Warn,
            "cached" => LabelKind::SecondaryOk,
            _ => LabelKind::Hint,
        },
    })
}

fn tree_finished_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}").unwrap_or_else(|_| ProgressStyle::default_bar())
}

fn shell_tree_heading(config: Config) -> String {
    label(config, "robo shell", LabelKind::Status)
}

fn display_shell_step(message: &str) -> &str {
    message.strip_prefix("shell: ").unwrap_or(message)
}

fn pad_display(value: String, width: usize) -> String {
    let padding = width.saturating_sub(console::measure_text_width(&value));
    format!("{value}{}", " ".repeat(padding))
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
    let runtime_input_fingerprints = runtime_input_fingerprints_for(Path::new("."));
    set_shell_env(
        envs,
        "ROBO_NIX_RUNTIME_INPUT_KEY",
        runtime_input_key_from_fingerprints(&runtime_input_fingerprints),
    );
    set_shell_env(
        envs,
        "ROBO_NIX_RUNTIME_INPUT_FILES",
        serialize_runtime_input_fingerprints(&runtime_input_fingerprints),
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
    shell_env_cache_key_for(Path::new("."))
}

pub(super) fn current_runtime_input_fingerprints_for(workspace: &Path) -> Vec<(String, String)> {
    runtime_input_fingerprints_for(workspace)
}

pub(super) fn current_runtime_input_key_from_fingerprints(
    fingerprints: &[(String, String)],
) -> String {
    runtime_input_key_from_fingerprints(fingerprints)
}

fn shell_env_cache_key_for(workspace: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    "shell-env-v2".hash(&mut hasher);
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

    runtime_input_fingerprints_for(workspace).hash(&mut hasher);
    hash_venv_cmake_prefixes(workspace, &mut hasher);

    format!("{:016x}", hasher.finish())
}

fn runtime_input_key_from_fingerprints(fingerprints: &[(String, String)]) -> String {
    let mut hasher = DefaultHasher::new();
    "runtime-input-v1".hash(&mut hasher);
    fingerprints.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn runtime_input_fingerprints_for(workspace: &Path) -> Vec<(String, String)> {
    let mut fingerprints = Vec::new();
    for path in [
        "flake.nix",
        "flake.lock",
        ".python-version",
        "pyproject.toml",
        "uv.lock",
    ] {
        fingerprints.push((path.to_string(), fingerprint_file(&workspace.join(path))));
    }
    let robo_fingerprint = normalized_robo_manifest(workspace)
        .map(|manifest| fingerprint_bytes(&manifest))
        .or_else(|| {
            parsed_nix_file(workspace, "robo.nix").map(|manifest| fingerprint_bytes(&manifest))
        })
        .unwrap_or_else(|| fingerprint_file(&workspace.join("robo.nix")));
    fingerprints.push(("robo.nix".to_string(), robo_fingerprint));
    fingerprints
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

pub(super) fn serialize_runtime_input_fingerprints(fingerprints: &[(String, String)]) -> String {
    serde_json::to_string(fingerprints).expect("runtime fingerprints should encode as JSON")
}

pub(super) fn parse_runtime_input_fingerprints(text: &str) -> Vec<(String, String)> {
    if let Ok(fingerprints) = serde_json::from_str(text) {
        return fingerprints;
    }

    text.split(';')
        .filter_map(|entry| entry.split_once('='))
        .map(|(path, hash)| (path.to_string(), hash.to_string()))
        .collect()
}

fn normalized_robo_manifest(workspace: &Path) -> Option<Vec<u8>> {
    let expr = r#"
      let
        spec = import ./robo.nix;
        provenance = spec.provenance or {};
        extraRuntimeLibraries = spec.extraRuntimeLibraries or [];
        runtimeLibraryName = library:
          if builtins.isAttrs library
          then library.pname or library.name or null
          else library;
      in builtins.toJSON {
        schemaVersion = if spec ? schemaVersion then toString spec.schemaVersion else null;
        envName = spec.envName or null;
        pythonVersion = spec.pythonVersion or null;
        cudaWheelVersion = spec.cudaWheelVersion or null;
        supportedSystems = spec.supportedSystems or [];
        workspaceRoot = spec.workspaceRoot or null;
        requirements = spec.requirements or [];
        components = spec.components or [];
        requiredDirectories = spec.requiredDirectories or [];
        requiredFiles = spec.requiredFiles or [];
        shellInit = spec.shellInit or "";
        bootstrap = spec.bootstrap or "";
        diagnostics = spec.diagnostics or "";
        extraRuntimeLibraries =
          if builtins.typeOf extraRuntimeLibraries == "lambda"
          then throw "function-valued extraRuntimeLibraries requires raw hash"
          else builtins.map runtimeLibraryName extraRuntimeLibraries;
        provenance = {
          profile = provenance.profile or null;
          sourceScripts = provenance.sourceScripts or [];
          suggestions = provenance.suggestions or [];
        };
      }
    "#;
    let output = Command::new("nix")
        .current_dir(workspace)
        .args([
            "--extra-experimental-features",
            "nix-command",
            "--extra-experimental-features",
            "flakes",
            "--no-warn-dirty",
            "--quiet",
            "eval",
            "--json",
            "--impure",
            "--expr",
            expr,
        ])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn parsed_nix_file(workspace: &Path, path: &str) -> Option<Vec<u8>> {
    let output = Command::new("nix-instantiate")
        .current_dir(workspace)
        .args(["--parse", path])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn hash_venv_cmake_prefixes<H: Hasher>(workspace: &Path, hasher: &mut H) {
    let pyvenv = workspace.join(".venv/pyvenv.cfg");
    pyvenv.hash(hasher);
    match fs::read(pyvenv) {
        Ok(bytes) => bytes.hash(hasher),
        Err(_) => 0_u8.hash(hasher),
    }

    let mut prefixes = Vec::new();
    let Ok(python_dirs) = fs::read_dir(workspace.join(".venv/lib")) else {
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

    #[test]
    fn live_tree_keeps_active_phase_on_its_own_line() {
        let config = Config {
            color: false,
            debug: false,
        };
        let state = ShellProgressTreeState {
            completed: vec![completed_tree_line(
                config,
                "shell: evaluating and realizing dev shell",
                Some("cached"),
                0,
                Duration::from_millis(79),
            )],
            active_message: "shell: launching shell".to_string(),
            active_started_at: Instant::now() - Duration::from_millis(812),
            evaluated_packages: HashSet::new(),
            details: VecDeque::from([
                "evaluating file '/workspace/flake.nix'".to_string(),
                "copying '/workspace/' to the store".to_string(),
            ]),
        };
        let live = render_live_tree(config, &state);

        assert_eq!(
            live,
            "robo shell\n  └ ✓ evaluating and realizing dev shell cached         79ms\n  └ ⠋ launching shell                                  812ms\n    evaluating file '/workspace/flake.nix'\n    copying '/workspace/' to the store"
        );
        assert!(!live.contains("robo shell  └"));
        assert!(!live.contains("robo shell └"));
    }

    #[test]
    fn finished_tree_preserves_completed_phase_tree() {
        let config = Config {
            color: false,
            debug: false,
        };
        let completed = vec![
            completed_tree_line(
                config,
                "shell: evaluating and realizing dev shell",
                Some("cached"),
                0,
                Duration::from_millis(13),
            ),
            completed_tree_line(
                config,
                "shell: launching shell",
                None,
                0,
                Duration::from_millis(0),
            ),
        ];

        assert_eq!(
            render_finished_tree(config, &completed, Duration::from_millis(14)),
            "✓ robo ready                                            14ms\n  └ ✓ evaluating and realizing dev shell cached         13ms"
        );
    }

    #[test]
    fn evaluation_phase_shows_unique_package_count() {
        let config = Config {
            color: false,
            debug: false,
        };
        let state = ShellProgressTreeState {
            completed: Vec::new(),
            active_message: "shell: evaluating and realizing dev shell".to_string(),
            active_started_at: Instant::now() - Duration::from_millis(812),
            evaluated_packages: HashSet::from([
                "/nix/store/source/pkgs/by-name/ja/jasper/package.nix".to_string(),
                "/nix/store/source/pkgs/by-name/li/libmng/package.nix".to_string(),
            ]),
            details: VecDeque::new(),
        };

        assert_eq!(
            render_live_tree(config, &state),
            "robo shell\n  └ ⠋ evaluating and realizing dev shell 2 packages    812ms"
        );
        assert_eq!(
            completed_tree_line(
                config,
                "shell: evaluating and realizing dev shell",
                None,
                2,
                Duration::from_secs(15),
            ),
            "  └ ✓ evaluating and realizing dev shell 2 packages    15.0s"
        );
    }

    #[test]
    fn active_child_spinner_advances_with_elapsed_time() {
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(0)), "⠋");
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(80)), "⠙");
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(160)), "⠹");
    }

    #[test]
    fn nix_progress_details_are_cleaned_for_display() {
        assert_eq!(
            nix_evaluated_package_path(
                b"evaluating file '/nix/store/source/pkgs/by-name/ja/jasper/package.nix'\n"
            )
            .as_deref(),
            Some("/nix/store/source/pkgs/by-name/ja/jasper/package.nix")
        );
        assert_eq!(
            nix_evaluated_package_path(b"evaluating file '/workspace/flake.nix'\n"),
            None
        );
        assert_eq!(
            shell_env_progress_detail(b"evaluating file '/workspace/flake.nix'\n"),
            None
        );
        assert_eq!(
            shell_env_progress_detail(
                br#"linking "/nix/store/source/file" to "/nix/store/.links/hash"
"#
            ),
            None
        );
    }

    #[test]
    fn runtime_input_fingerprints_round_trip() {
        let fingerprints = vec![
            ("pyproject.toml".to_string(), "abc".to_string()),
            ("robo.nix".to_string(), "def".to_string()),
        ];

        assert_eq!(
            parse_runtime_input_fingerprints(&serialize_runtime_input_fingerprints(&fingerprints)),
            fingerprints
        );
    }

    #[test]
    fn runtime_input_fingerprints_parse_legacy_env_value() {
        assert_eq!(
            parse_runtime_input_fingerprints("pyproject.toml=abc;robo.nix=def"),
            vec![
                ("pyproject.toml".to_string(), "abc".to_string()),
                ("robo.nix".to_string(), "def".to_string()),
            ]
        );
    }
}
