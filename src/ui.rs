use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::env;
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

pub(crate) fn row(config: Config, marker: &str, action: &str, detail: &str) {
    println!(
        "  {} {:<8} {}",
        label(config, marker, LabelKind::Ok),
        label(config, action, LabelKind::Hint),
        inline(config, detail)
    );
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

pub(crate) fn output_with_spinner(
    config: Config,
    command: &mut Command,
    message: &str,
) -> Result<Output, std::io::Error> {
    if config.debug
        || env::var_os("NO_COLOR").is_some()
        || env::var_os("ROBO_NIX_NO_SPINNER").is_some()
        || !std::io::stderr().is_terminal()
    {
        status(config, message);
        return command.output();
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let cursor = HiddenCursor::new();
    let spinner = spinner(config, message);
    let started_at = Instant::now();
    let output = command.output();
    keep_spinner_visible(started_at);
    spinner.finish_and_clear();
    cursor.show();
    output
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

struct HiddenCursor {
    hidden: bool,
}

impl HiddenCursor {
    fn new() -> Self {
        eprint!("\x1b[?25l");
        let _ = std::io::stderr().flush();
        Self { hidden: true }
    }

    fn show(mut self) {
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
