use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::IsTerminal;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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
    Why,
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
        LabelKind::Why => style(text).magenta().bold().to_string(),
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

pub(crate) fn ok(config: Config, message: &str) {
    eprintln!("{} {}", label(config, "ok:", LabelKind::Ok), inline(config, message));
}

pub(crate) fn warn(config: Config, message: &str) {
    eprintln!("{} {}", label(config, "warn:", LabelKind::Warn), inline(config, message));
}

pub(crate) fn error(config: Config, message: &str) {
    eprintln!("{} {}", label(config, "error:", LabelKind::Error), inline(config, message));
}

pub(crate) fn hint(config: Config, message: &str) {
    eprintln!("{} {}", label(config, "hint:", LabelKind::Hint), inline(config, message));
}

pub(crate) fn section(config: Config, heading: &str) {
    println!("{}", label(config, heading, LabelKind::Status));
}

pub(crate) fn field(config: Config, name: &str, value: &str) {
    println!("  {}={}", label(config, name, LabelKind::Hint), value);
}

pub(crate) fn section_err(config: Config, heading: &str) {
    eprintln!("{}", label(config, heading, LabelKind::Status));
}

pub(crate) fn field_err(config: Config, name: &str, value: &str) {
    eprintln!("  {}={}", label(config, name, LabelKind::Hint), value);
}

pub(crate) fn command_row_err(config: Config, command: &str) {
    eprintln!("  {}", label(config, command, LabelKind::Command));
}

pub(crate) fn output_with_spinner(
    config: Config,
    command: &mut Command,
    message: &str,
) -> Result<Output, std::io::Error> {
    if config.debug || !std::io::stderr().is_terminal() {
        status(config, message);
        return command.output();
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let spinner = spinner(config, message);
    let started_at = Instant::now();
    let output = command.output();
    keep_spinner_visible(started_at);
    spinner.finish_and_clear();
    output
}

pub(crate) struct UiProgress {
    config: Config,
    bar: Option<ProgressBar>,
    current: u64,
    total: u64,
}

pub(crate) struct UiSpinner {
    config: Config,
    bar: Option<ProgressBar>,
}

impl UiSpinner {
    pub(crate) fn new(config: Config, message: &str) -> Self {
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self { config, bar: None };
        }

        Self {
            config,
            bar: Some(spinner(config, message)),
        }
    }

    pub(crate) fn finish(&mut self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }

    pub(crate) fn set_message(&self, message: &str) {
        if let Some(bar) = &self.bar {
            bar.set_message(status_message(self.config, message));
        }
    }
}

impl Drop for UiSpinner {
    fn drop(&mut self) {
        self.finish();
    }
}

impl UiProgress {
    pub(crate) fn new(config: Config, total: u64, message: &str) -> Self {
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self {
                config,
                bar: None,
                current: 0,
                total,
            };
        }

        let bar = ProgressBar::new(total);
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.cyan} [{bar:20.cyan/blue}] {pos:.dim}/{len:.dim} {msg}",
            )
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("=>-")
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        bar.set_message(status_message(config, message));
        bar.enable_steady_tick(Duration::from_millis(80));

        Self {
            config,
            bar: Some(bar),
            current: 0,
            total,
        }
    }

    pub(crate) fn step(&mut self, message: &str) {
        self.current = (self.current + 1).min(self.total);
        if let Some(bar) = &self.bar {
            bar.set_position(self.current);
            bar.set_message(status_message(self.config, message));
        } else {
            status(self.config, message);
        }
    }

    pub(crate) fn set(&self, message: &str) {
        if let Some(bar) = &self.bar {
            bar.set_message(status_message(self.config, message));
        } else {
            status(self.config, message);
        }
    }

    pub(crate) fn output(
        &mut self,
        command: &mut Command,
        message: &str,
    ) -> Result<Output, std::io::Error> {
        self.step(message);
        if self.bar.is_some() {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        command.output()
    }

    pub(crate) fn output_current(
        &self,
        command: &mut Command,
        message: &str,
    ) -> Result<Output, std::io::Error> {
        self.set(message);
        if self.bar.is_some() {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        command.output()
    }

    pub(crate) fn suspend<T>(&self, operation: impl FnOnce() -> T) -> T {
        match &self.bar {
            Some(bar) => bar.suspend(operation),
            None => operation(),
        }
    }

    pub(crate) fn finish(&mut self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}

fn spinner(config: Config, message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_draw_target(ProgressDrawTarget::stderr());
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {elapsed_precise:.dim} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(status_message(config, message));
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
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

fn keep_spinner_visible(started_at: Instant) {
    let minimum = Duration::from_millis(450);
    let elapsed = started_at.elapsed();
    if elapsed < minimum {
        std::thread::sleep(minimum - elapsed);
    }
}
