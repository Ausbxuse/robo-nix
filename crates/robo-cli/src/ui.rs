use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::cell::Cell;
use std::io::{IsTerminal, Write};
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
    SecondaryOk,
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
        LabelKind::SecondaryOk => style(text).green().dim().to_string(),
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
    let _cursor = HiddenCursor::new();
    let spinner = spinner(config, message);
    let started_at = Instant::now();
    let output = command.output();
    keep_spinner_visible(started_at);
    spinner.finish_and_clear();
    _cursor.show();
    output
}

pub(crate) struct UiProgress {
    config: Config,
    bar: Option<ProgressBar>,
    _cursor: Option<HiddenCursor>,
}

pub(crate) struct UiSpinner {
    bar: Option<ProgressBar>,
    _cursor: Option<HiddenCursor>,
}

impl UiSpinner {
    pub(crate) fn new(config: Config, message: &str) -> Self {
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self {
                bar: None,
                _cursor: None,
            };
        }

        Self {
            bar: Some(spinner(config, message)),
            _cursor: Some(HiddenCursor::new()),
        }
    }

    pub(crate) fn finish(&mut self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
        if let Some(cursor) = &self._cursor {
            cursor.show();
        }
    }
}

impl Drop for UiSpinner {
    fn drop(&mut self) {
        self.finish();
    }
}

impl UiProgress {
    pub(crate) fn new(config: Config, _total: u64, message: &str) -> Self {
        if config.debug || !std::io::stderr().is_terminal() {
            status(config, message);
            return Self {
                config,
                bar: None,
                _cursor: None,
            };
        }

        Self {
            config,
            bar: Some(spinner(config, message)),
            _cursor: Some(HiddenCursor::new()),
        }
    }

    pub(crate) fn step(&mut self, message: &str) {
        if let Some(bar) = &self.bar {
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
        if let Some(cursor) = &self._cursor {
            cursor.show();
        }
    }
}

fn spinner(config: Config, message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_draw_target(ProgressDrawTarget::stderr());
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg} {elapsed_precise:.dim}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(status_message(config, message));
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

pub(crate) struct HiddenCursor {
    hidden: Cell<bool>,
}

impl HiddenCursor {
    pub(crate) fn new() -> Self {
        eprint!("\x1b[?25l");
        let _ = std::io::stderr().flush();
        Self {
            hidden: Cell::new(true),
        }
    }

    pub(crate) fn show(&self) {
        if self.hidden.replace(false) {
            eprint!("\x1b[?25h");
            let _ = std::io::stderr().flush();
        }
    }
}

impl Drop for HiddenCursor {
    fn drop(&mut self) {
        self.show();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_short_human_durations_as_milliseconds() {
        assert_eq!(human_duration(Duration::from_millis(42)), "42ms");
    }

    #[test]
    fn formats_second_human_durations_with_one_decimal() {
        assert_eq!(human_duration(Duration::from_millis(1250)), "1.2s");
    }

    #[test]
    fn formats_long_human_durations_as_minutes_and_seconds() {
        assert_eq!(human_duration(Duration::from_secs(125)), "2m 5s");
    }
}
