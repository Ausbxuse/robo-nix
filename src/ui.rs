use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::collections::{HashSet, VecDeque};
use std::env;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::process::{Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TREE_SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TREE_STEP_WIDTH: usize = 34;
const TREE_META_WIDTH: usize = 12;
const TREE_DURATION_WIDTH: usize = 6;
const TREE_READY_WIDTH: usize = 51;

#[derive(Clone, Copy)]
pub(crate) struct Config {
    pub(crate) color: bool,
    pub(crate) debug: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum LabelKind {
    Status,
    Ok,
    Warn,
    Error,
    Hint,
    Debug,
    Command,
}

pub(crate) fn label(config: Config, text: &str, kind: LabelKind) -> String {
    if !config.color {
        return text.to_string();
    }

    match kind {
        LabelKind::Status => style(text).cyan().bold().to_string(),
        LabelKind::Ok => style(text).green().bold().to_string(),
        LabelKind::Warn => style(text).yellow().bold().to_string(),
        LabelKind::Error => style(text).red().bold().to_string(),
        LabelKind::Hint => style(text).dim().to_string(),
        LabelKind::Debug => style(text).magenta().bold().to_string(),
        LabelKind::Command => style(text).green().to_string(),
    }
}

pub(crate) fn inline(config: Config, text: &str) -> String {
    if !config.color {
        return text.to_string();
    }

    let mut out = String::new();
    let mut rest = text;
    while let Some((prefix, delimiter, after)) = next_inline_delimiter(rest) {
        out.push_str(prefix);
        if let Some(end) = after.find(delimiter) {
            let (body, tail) = after.split_at(end);
            out.push_str(&label(
                config,
                &format!("{delimiter}{body}{delimiter}"),
                LabelKind::Command,
            ));
            rest = &tail[delimiter.len_utf8()..];
        } else {
            out.push(delimiter);
            out.push_str(after);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn next_inline_delimiter(text: &str) -> Option<(&str, char, &str)> {
    let quote = text.find('\'');
    let tick = text.find('`');
    let (index, delimiter) = match (quote, tick) {
        (Some(quote), Some(tick)) if quote < tick => (quote, '\''),
        (Some(_), Some(tick)) => (tick, '`'),
        (Some(quote), None) => (quote, '\''),
        (None, Some(tick)) => (tick, '`'),
        (None, None) => return None,
    };
    let after = &text[index + delimiter.len_utf8()..];
    Some((&text[..index], delimiter, after))
}

pub(crate) fn status(config: Config, message: &str) {
    eprintln!("{}", status_message(config, message));
}

pub(crate) fn error(config: Config, message: &str) {
    eprintln!(
        "{} {}",
        label(config, "error:", LabelKind::Error),
        inline(config, message)
    );
}

pub(crate) fn hint(config: Config, message: &str) {
    eprintln!(
        "{} {}",
        label(config, "hint:", LabelKind::Hint),
        inline(config, message)
    );
}

pub(crate) fn section(config: Config, heading: &str) {
    println!("{}", label(config, heading, LabelKind::Status));
}

pub(crate) fn help_row(config: Config, command: &str, detail: &str) {
    let padding = " ".repeat(24usize.saturating_sub(console::measure_text_width(command)));
    println!(
        "  {}{} {}",
        label(config, command, LabelKind::Command),
        padding,
        label(config, detail, LabelKind::Hint)
    );
}

pub(crate) fn row(config: Config, marker: &str, action: &str, detail: &str) {
    println!(
        "  {} {:<8} {}",
        label(config, marker, marker_kind(marker)),
        label(config, action, LabelKind::Hint),
        inline(config, detail)
    );
}

pub(crate) fn row_err(config: Config, marker: &str, action: &str, detail: &str) {
    eprintln!(
        "  {} {:<8} {}",
        label(config, marker, marker_kind(marker)),
        label(config, action, LabelKind::Hint),
        inline(config, detail)
    );
}

pub(crate) fn list_item(config: Config, detail: &str) {
    println!("  {}", inline(config, detail));
}

pub(crate) fn detail(config: Config, detail: &str) {
    println!("    {}", inline(config, detail));
}

pub(crate) fn success(config: Config, subject: &str, detail: &str) {
    println!(
        "  {} {:<14} {}",
        label(config, "✓", LabelKind::Ok),
        subject,
        inline(config, detail)
    );
}

pub(crate) fn attention(config: Config, detail: &str) {
    println!(
        "  {} {}",
        label(config, "!", LabelKind::Warn),
        inline(config, detail)
    );
}

pub(crate) fn debug(config: Config, message: &str) {
    eprintln!(
        "{} {}",
        label(config, "debug:", LabelKind::Debug),
        inline(config, message)
    );
}

fn marker_kind(marker: &str) -> LabelKind {
    match marker {
        "!" => LabelKind::Warn,
        _ => LabelKind::Ok,
    }
}

pub(crate) fn output_with_tree(
    config: Config,
    command: &mut Command,
    root: &str,
    message: &str,
) -> Result<Output, std::io::Error> {
    output_with_tree_steps(config, command, root, message, Vec::new())
}

pub(crate) struct ProgressStep {
    pub(crate) message: String,
    pub(crate) suffix: Option<String>,
    pub(crate) duration: Duration,
}

impl ProgressStep {
    pub(crate) fn instant(message: impl Into<String>, suffix: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suffix: Some(suffix.into()),
            duration: Duration::from_millis(0),
        }
    }
}

pub(crate) fn output_with_tree_steps(
    config: Config,
    command: &mut Command,
    root: &str,
    message: &str,
    completed_steps: Vec<ProgressStep>,
) -> Result<Output, std::io::Error> {
    if should_use_plain_progress(config) {
        for step in &completed_steps {
            let suffix = step
                .suffix
                .as_deref()
                .map(|suffix| format!(" {suffix}"))
                .unwrap_or_default();
            status(config, &format!("{}{}", step.message, suffix));
        }
        status(config, message);
        return command.output();
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let tree = ProgressTree::new(config, root, message, completed_steps);
    let started_at = Instant::now();

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            tree.finish_clear();
            return Err(err);
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| other_io("failed to capture command stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| other_io("failed to capture command stderr"))?;

    let stdout_reader = thread::spawn(move || {
        let mut stdout = stdout;
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let detail_sink = tree.detail_sink();
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
            detail_sink.detail(&line);
        }
        Ok::<Vec<u8>, std::io::Error>(bytes)
    });

    let status = child.wait()?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| other_io("failed to join command stdout reader"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| other_io("failed to join command stderr reader"))??;

    let output = Output {
        status,
        stdout,
        stderr,
    };
    if output.status.success() {
        tree.finish_ready(started_at.elapsed());
    } else {
        tree.finish_clear();
    }
    Ok(output)
}

pub(crate) fn output_cached_tree(config: Config, message: &str) {
    if should_use_plain_progress(config) {
        status(config, &format!("{message} cached"));
        return;
    }

    let completed = vec![completed_tree_line(
        config,
        message,
        Some("cached"),
        0,
        Duration::from_millis(0),
    )];
    eprintln!(
        "{}",
        render_finished_tree(config, &completed, Duration::from_millis(0))
    );
}

pub(crate) fn output_with_spinner(
    config: Config,
    command: &mut Command,
    message: &str,
) -> Result<Output, std::io::Error> {
    if should_use_plain_progress(config) {
        status(config, message);
        return command.output();
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut cursor = HiddenCursor::new();
    let bar = spinner_bar(config, message);
    let output = command.output();
    bar.finish_and_clear();
    cursor.show();
    output
}

fn should_use_plain_progress(config: Config) -> bool {
    config.debug
        || env::var_os("NO_COLOR").is_some()
        || env::var_os("ROBO_NIX_NO_SPINNER").is_some()
        || !std::io::stderr().is_terminal()
}

fn spinner_bar(config: Config, message: &str) -> ProgressBar {
    let bar = tree_bar();
    bar.set_message(status_message(config, message));
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

struct HiddenCursor {
    hidden: bool,
}

impl HiddenCursor {
    fn new() -> Self {
        eprint!("\x1b[?25l");
        let _ = std::io::stderr().flush();
        Self { hidden: true }
    }

    fn show(&mut self) {
        if self.hidden {
            eprint!("\x1b[?25h");
            let _ = std::io::stderr().flush();
            self.hidden = false;
        }
    }
}

impl Drop for HiddenCursor {
    fn drop(&mut self) {
        if self.hidden {
            eprint!("\x1b[?25h");
            let _ = std::io::stderr().flush();
            self.hidden = false;
        }
    }
}

struct ProgressTree {
    config: Config,
    bar: ProgressBar,
    state: Arc<Mutex<ProgressTreeState>>,
    stop_ticker: Arc<AtomicBool>,
    ticker: Mutex<Option<JoinHandle<()>>>,
    cursor: Mutex<HiddenCursor>,
}

struct ProgressTreeState {
    root: String,
    completed: Vec<String>,
    active_message: String,
    active_started_at: Instant,
    evaluated_packages: HashSet<String>,
    details: VecDeque<String>,
}

struct ProgressTreeDetailSink {
    config: Config,
    bar: ProgressBar,
    state: Arc<Mutex<ProgressTreeState>>,
}

impl ProgressTreeDetailSink {
    fn detail(&self, bytes: &[u8]) {
        let mut state = self.state.lock().expect("progress tree state poisoned");
        if let Some(package) = nix_evaluated_package_path(bytes) {
            state.evaluated_packages.insert(package);
        }
        if let Some(detail) = progress_detail(bytes) {
            if state.details.back() != Some(&detail) {
                state.details.push_back(detail);
                while state.details.len() > 4 {
                    state.details.pop_front();
                }
            }
        }
        self.bar
            .set_message(render_live_tree(self.config, &state, Instant::now()));
    }
}

impl ProgressTree {
    fn new(config: Config, root: &str, message: &str, completed_steps: Vec<ProgressStep>) -> Self {
        let bar = tree_bar();
        let state = Arc::new(Mutex::new(ProgressTreeState {
            root: root.to_string(),
            completed: completed_steps
                .into_iter()
                .map(|step| {
                    completed_tree_line(
                        config,
                        &step.message,
                        step.suffix.as_deref(),
                        0,
                        step.duration,
                    )
                })
                .collect(),
            active_message: message.to_string(),
            active_started_at: Instant::now(),
            evaluated_packages: HashSet::new(),
            details: VecDeque::new(),
        }));
        let stop_ticker = Arc::new(AtomicBool::new(false));
        let ticker = spawn_tree_ticker(config, &bar, &state, &stop_ticker);
        let tree = Self {
            config,
            bar,
            state,
            stop_ticker,
            ticker: Mutex::new(Some(ticker)),
            cursor: Mutex::new(HiddenCursor::new()),
        };
        tree.render_live();
        tree.bar.enable_steady_tick(Duration::from_millis(80));
        tree
    }

    fn detail_sink(&self) -> ProgressTreeDetailSink {
        ProgressTreeDetailSink {
            config: self.config,
            bar: self.bar.clone(),
            state: Arc::clone(&self.state),
        }
    }

    fn finish_ready(&self, duration: Duration) {
        self.finish_active_child(None);
        self.stop_ticker();
        let state = self.state.lock().expect("progress tree state poisoned");
        let finished = render_finished_tree(self.config, &state.completed, duration);
        self.bar.finish_and_clear();
        eprintln!("{finished}");
        self.show_cursor();
    }

    fn finish_clear(&self) {
        self.stop_ticker();
        self.bar.finish_and_clear();
        self.show_cursor();
    }

    fn finish_active_child(&self, suffix: Option<&str>) {
        let mut state = self.state.lock().expect("progress tree state poisoned");
        let evaluated_packages = state.evaluated_packages.len();
        let active_message = state.active_message.clone();
        let elapsed = state.active_started_at.elapsed();
        state.completed.push(completed_tree_line(
            self.config,
            &active_message,
            suffix,
            evaluated_packages,
            elapsed,
        ));
        state.evaluated_packages.clear();
        state.details.clear();
    }

    fn render_live(&self) {
        let state = self.state.lock().expect("progress tree state poisoned");
        self.bar
            .set_message(render_live_tree(self.config, &state, Instant::now()));
    }

    fn stop_ticker(&self) {
        self.stop_ticker.store(true, Ordering::Relaxed);
        if let Some(ticker) = self
            .ticker
            .lock()
            .expect("progress tree ticker lock poisoned")
            .take()
        {
            let _ = ticker.join();
        }
    }

    fn show_cursor(&self) {
        self.cursor
            .lock()
            .expect("progress tree cursor lock poisoned")
            .show();
    }
}

impl Drop for ProgressTree {
    fn drop(&mut self) {
        self.stop_ticker();
        self.show_cursor();
    }
}

fn spawn_tree_ticker(
    config: Config,
    bar: &ProgressBar,
    state: &Arc<Mutex<ProgressTreeState>>,
    stop_ticker: &Arc<AtomicBool>,
) -> JoinHandle<()> {
    let bar = bar.clone();
    let state = Arc::clone(state);
    let stop_ticker = Arc::clone(stop_ticker);
    thread::spawn(move || {
        while !stop_ticker.load(Ordering::Relaxed) {
            if let Ok(state) = state.lock() {
                bar.set_message(render_live_tree(config, &state, Instant::now()));
            }
            thread::sleep(Duration::from_millis(80));
        }
    })
}

fn tree_bar() -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_draw_target(ProgressDrawTarget::stderr());
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(TREE_SPINNER_FRAMES),
    );
    bar
}

fn render_live_tree(config: Config, state: &ProgressTreeState, now: Instant) -> String {
    let mut lines = vec![label(config, &state.root, LabelKind::Status)];
    lines.extend(
        state
            .completed
            .iter()
            .filter(|line| !line.is_empty())
            .cloned(),
    );
    if !state.active_message.is_empty() {
        let elapsed = now.saturating_duration_since(state.active_started_at);
        lines.push(format!(
            "  {} {} {}",
            label(config, "└", LabelKind::Hint),
            label(config, active_spinner_for_elapsed(elapsed), LabelKind::Hint),
            tree_fields(
                config,
                &state.active_message,
                tree_metadata(&state.active_message, None, state.evaluated_packages.len()),
                elapsed,
            )
        ));
    }
    for detail in &state.details {
        lines.push(format!("    {}", label(config, detail, LabelKind::Hint)));
    }
    lines.join("\n")
}

fn render_finished_tree(config: Config, completed: &[String], duration: Duration) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        label(config, "✓", LabelKind::Ok),
        pad_display(label(config, "robo ready", LabelKind::Ok), TREE_READY_WIDTH),
        label(
            config,
            &format!("{:>TREE_DURATION_WIDTH$}", human_duration(duration)),
            LabelKind::Ok,
        )
    )];
    lines.extend(completed.iter().filter(|line| !line.is_empty()).cloned());
    lines.join("\n")
}

fn completed_tree_line(
    config: Config,
    message: &str,
    suffix: Option<&str>,
    evaluated_packages: usize,
    duration: Duration,
) -> String {
    format!(
        "  {} {} {}",
        label(config, "└", LabelKind::Hint),
        label(config, "✓", LabelKind::Ok),
        tree_fields(
            config,
            message,
            tree_metadata(message, suffix, evaluated_packages),
            duration,
        )
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
        pad_display(inline(config, display_tree_step(message)), TREE_STEP_WIDTH),
        pad_display(metadata, TREE_META_WIDTH),
        label(
            config,
            &format!("{:>TREE_DURATION_WIDTH$}", human_duration(duration)),
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
    if evaluated_packages > 0
        && (message.ends_with(": evaluating and realizing dev shell")
            || message.ends_with(": evaluating runtime shell"))
    {
        return Some(TreeMetadata {
            text: format!("{evaluated_packages} packages"),
            kind: LabelKind::Status,
        });
    }
    suffix.map(|suffix| TreeMetadata {
        text: suffix.to_string(),
        kind: LabelKind::Hint,
    })
}

fn display_tree_step(message: &str) -> &str {
    message
        .split_once(": ")
        .map(|(_, step)| step)
        .unwrap_or(message)
}

fn active_spinner_for_elapsed(elapsed: Duration) -> &'static str {
    let frame = (elapsed.as_millis() / 80) as usize % TREE_SPINNER_FRAMES.len();
    TREE_SPINNER_FRAMES[frame]
}

fn pad_display(value: String, width: usize) -> String {
    let padding = width.saturating_sub(console::measure_text_width(&value));
    format!("{value}{}", " ".repeat(padding))
}

fn progress_detail(bytes: &[u8]) -> Option<String> {
    let detail = clean_progress_text(&nix_log_text(bytes)?);
    if detail.is_empty() {
        return None;
    }
    if ignored_progress_detail(&detail) {
        return None;
    }
    Some(truncate_progress_detail(&nix_activity_detail(&detail)))
}

fn nix_evaluated_package_path(bytes: &[u8]) -> Option<String> {
    let detail = clean_progress_text(&nix_log_text(bytes)?);
    let path = detail
        .strip_prefix("evaluating file '")?
        .strip_suffix("'")?;
    path.ends_with("/package.nix").then(|| path.to_string())
}

fn nix_log_text(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.rsplit('\r').next().unwrap_or(&text).trim();
    if text.is_empty() {
        return None;
    }
    if let Some(json) = text.strip_prefix("@nix ") {
        return json_string_field(json, "text").or_else(|| json_string_field(json, "msg"));
    }
    Some(text.to_string())
}

fn ignored_progress_detail(detail: &str) -> bool {
    detail.starts_with("warning: Git tree '")
        || detail.starts_with("linking ")
        || detail.starts_with("evaluating file '")
}

fn nix_activity_detail(detail: &str) -> String {
    if let Some(path) = single_quoted_after(detail, "evaluating derivation ") {
        return format!("evaluating {}", installable_or_store_name(path));
    }
    if let Some(path) = single_quoted_after(detail, "building ") {
        return format!("building {}", installable_or_store_name(path));
    }
    if let Some(path) = single_quoted_after(detail, "copying ") {
        if detail.ends_with(" to the store") {
            return format!("copying {} to the store", display_copy_source(path));
        }
    }
    if let Some(path) = single_quoted_after(detail, "copying path ") {
        let name = installable_or_store_name(path);
        if let Some(source) = single_quoted_after_marker(detail, " from ") {
            return format!("fetching {name} from {}", display_store_source(source));
        }
        return format!("copying {name}");
    }
    if let Some(path) = single_quoted_after(detail, "querying info about ") {
        return format!("querying {}", installable_or_store_name(path));
    }
    if let Some(count) = these_count(detail, " derivations will be built:") {
        return format!("planning {count} builds");
    }
    if let Some(count) = these_count(detail, " paths will be fetched") {
        return format!("planning {count} fetches");
    }
    if let Some(count) = these_count(detail, " paths will be copied") {
        return format!("planning {count} store copies");
    }
    match detail {
        "this derivation will be built:" => "planning 1 build".to_string(),
        "this path will be fetched:" => "planning 1 fetch".to_string(),
        "this path will be copied:" => "planning 1 store copy".to_string(),
        _ => detail.to_string(),
    }
}

fn single_quoted_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(prefix)?;
    let rest = rest.strip_prefix('\'')?;
    rest.split_once('\'').map(|(quoted, _)| quoted)
}

fn single_quoted_after_marker<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = text.split_once(marker)?.1;
    let rest = rest.strip_prefix('\'')?;
    rest.split_once('\'').map(|(quoted, _)| quoted)
}

fn these_count(text: &str, suffix: &str) -> Option<String> {
    let rest = text.strip_prefix("these ")?;
    let (count, _) = rest.split_once(suffix)?;
    (!count.is_empty() && count.chars().all(|ch| ch.is_ascii_digit())).then(|| count.to_string())
}

fn installable_or_store_name(path: &str) -> String {
    if let Some(name) = store_path_name(path) {
        return name.to_string();
    }
    path.strip_prefix("git+file://")
        .and_then(|path| path.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn display_copy_source(path: &str) -> String {
    if path == "/" {
        return "source".to_string();
    }
    if let Some(name) = store_path_name(path) {
        return name.to_string();
    }
    if path.ends_with('/') {
        return "workspace".to_string();
    }
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn display_store_source(source: &str) -> String {
    source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
        .and_then(|source| source.split('/').next())
        .filter(|host| !host.is_empty())
        .unwrap_or(source)
        .to_string()
}

fn store_path_name(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/nix/store/")?;
    let (_, name) = rest.split_once('-')?;
    (!name.is_empty()).then_some(name)
}

fn clean_progress_text(text: &str) -> String {
    strip_ansi(text)
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .collect()
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        if chars.next_if_eq(&'[').is_some() {
            for ch in chars.by_ref() {
                if ('@'..='~').contains(&ch) {
                    break;
                }
            }
        }
    }
    out
}

fn json_string_field(text: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let after = text.split_once(&needle)?.1.trim_start();
    let after = after.strip_prefix('"')?;
    decode_json_string(after)
}

fn decode_json_string(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    let mut code = String::new();
                    for _ in 0..4 {
                        code.push(chars.next()?);
                    }
                    let code = u32::from_str_radix(&code, 16).ok()?;
                    out.push(char::from_u32(code)?);
                }
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

fn truncate_progress_detail(detail: &str) -> String {
    const MAX_DETAIL_CHARS: usize = 110;
    if detail.chars().count() <= MAX_DETAIL_CHARS {
        return detail.to_string();
    }
    let mut truncated = detail
        .chars()
        .take(MAX_DETAIL_CHARS - 1)
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn status_message(config: Config, message: &str) -> String {
    let Some((phase, rest)) = message.split_once(": ") else {
        return inline(config, message);
    };

    format!(
        "{} {}",
        label(config, &format!("{phase}:"), LabelKind::Status),
        inline(config, rest)
    )
}

pub(crate) fn human_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        return format!("{millis}ms");
    }

    let seconds = duration.as_secs_f64();
    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }

    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}m {seconds}s")
}

fn other_io(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_tree_keeps_active_phase_nested() {
        let config = Config {
            color: false,
            debug: false,
        };
        let started = Instant::now();
        let state = ProgressTreeState {
            root: "robo shell".to_string(),
            completed: vec![completed_tree_line(
                config,
                "shell: preparing runtime files",
                None,
                0,
                Duration::from_millis(79),
            )],
            active_message: "shell: evaluating and realizing dev shell".to_string(),
            active_started_at: started,
            evaluated_packages: HashSet::from([
                "/nix/store/source/pkgs/by-name/li/libmng/package.nix".to_string(),
                "/nix/store/source/pkgs/by-name/ja/jasper/package.nix".to_string(),
            ]),
            details: VecDeque::from(["copying '/workspace/' to the store".to_string()]),
        };

        assert_eq!(
            render_live_tree(config, &state, started + Duration::from_millis(812)),
            "robo shell\n  └ ✓ preparing runtime files                           79ms\n  └ ⠋ evaluating and realizing dev shell 2 packages    812ms\n    copying '/workspace/' to the store"
        );
    }

    #[test]
    fn finished_tree_preserves_completed_children() {
        let config = Config {
            color: false,
            debug: false,
        };
        let completed = vec![completed_tree_line(
            config,
            "shell: evaluating and realizing dev shell",
            Some("cached"),
            0,
            Duration::from_millis(13),
        )];

        assert_eq!(
            render_finished_tree(config, &completed, Duration::from_millis(14)),
            "✓ robo ready                                            14ms\n  └ ✓ evaluating and realizing dev shell cached         13ms"
        );
    }

    #[test]
    fn progress_details_are_cleaned_for_display() {
        assert_eq!(
            progress_detail(b"\revaluating file '/nix/store/x/package.nix'\n"),
            None
        );
        assert_eq!(
            progress_detail(b"warning: Git tree '/workspace/project' is dirty\n"),
            None
        );
        assert_eq!(
            progress_detail(b"\x1b[35;1mwarning:\x1b[0m Git tree '/workspace/project' is dirty\n"),
            None
        );
        assert_eq!(
            progress_detail(b"\rcopying '/workspace/' to the store\n"),
            Some("copying workspace to the store".to_string())
        );
        assert_eq!(
            progress_detail(
                b"building '/nix/store/abc12345678901234567890123456789012-glibc-2.40.drv'...\n"
            ),
            Some("building glibc-2.40.drv".to_string())
        );
        assert_eq!(
            progress_detail(b"copying path '/nix/store/abc12345678901234567890123456789012-python3-3.11.13' from 'https://cache.nixos.org'\n"),
            Some("fetching python3-3.11.13 from cache.nixos.org".to_string())
        );
        assert_eq!(
            progress_detail(
                b"these 27 paths will be fetched (123 MiB download, 456 MiB unpacked):\n"
            ),
            Some("planning 27 fetches".to_string())
        );
        assert_eq!(
            progress_detail(br#"@nix {"action":"start","id":1,"level":4,"parent":0,"text":"copying '/nix/store/abc12345678901234567890123456789012-source' to the store","type":0}"#),
            Some("copying source to the store".to_string())
        );
        assert_eq!(
            nix_evaluated_package_path(br#"@nix {"action":"msg","level":4,"msg":"evaluating file '/nix/store/src/pkgs/by-name/gl/glfw/package.nix'"}"#),
            Some("/nix/store/src/pkgs/by-name/gl/glfw/package.nix".to_string())
        );
    }

    #[test]
    fn active_child_spinner_advances_with_elapsed_time() {
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(0)), "⠋");
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(80)), "⠙");
        assert_eq!(active_spinner_for_elapsed(Duration::from_millis(160)), "⠹");
    }

    #[test]
    fn formats_human_durations() {
        assert_eq!(human_duration(Duration::from_millis(42)), "42ms");
        assert_eq!(human_duration(Duration::from_millis(1250)), "1.2s");
        assert_eq!(human_duration(Duration::from_secs(125)), "2m 5s");
    }
}
